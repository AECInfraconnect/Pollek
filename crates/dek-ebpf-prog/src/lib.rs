// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

#![no_std]
#![no_main]

//! dek-ebpf-prog — BPF programs for the DEK network Control Point.
//!
//! Ported to aya-ebpf (aya 0.13). Two programs:
//!  - `dek_dns_capture` (cgroup/skb): REAL DNS observation. Parses IP+UDP at the
//!    L3 layer (cgroup_skb sees packets WITHOUT the Ethernet header), filters
//!    UDP port 53, copies the DNS payload into a ring buffer for userspace to
//!    parse with hickory. Observe-only: always returns 1 (pass), never drops.
//!  - `dek_connect4` (cgroup/connect4): IP/port egress guardrail (kept).
//!
//! Verifier note: eBPF is compiled + verified at load time. Bounded copies use
//! a power-of-two mask so the verifier can prove in-bounds access. If the
//! verifier rejects a build, the masked-length copy below is the place to tune.

use aya_ebpf::{
    helpers::{bpf_get_current_cgroup_id, bpf_get_current_pid_tgid, bpf_ktime_get_ns},
    macros::{cgroup_skb, cgroup_sock_addr, map},
    maps::{HashMap, LpmTrie, LruHashMap, PerCpuArray, RingBuf},
    programs::{SkBuffContext, SockAddrContext},
};
use dek_ebpf_common::{
    DnsCaptureEvent, EgressEvent, Ipv4LpmKey, PolicyVerdict, CGROUP_MAP_CAPACITY,
    DNS_PAYLOAD_MAX, LPM_MAP_CAPACITY, PORTS_MAP_CAPACITY,
    DekIp4Key, DekDnsCacheValue, DekMetrics, DEK_DNS_CACHE_MAX_ENTRIES,
};

const DNS_PORT: u16 = 53;
const IPPROTO_UDP: u8 = 17;
const IPV6_HDR_LEN: usize = 40;
const UDP_HDR_LEN: usize = 8;

// ------------------------------- maps -------------------------------

#[map]
static VERDICT_MAP: LpmTrie<u32, PolicyVerdict> =
    LpmTrie::with_max_entries(LPM_MAP_CAPACITY, 0);

#[map]
static PORTS_MAP: HashMap<u16, PolicyVerdict> = HashMap::with_max_entries(PORTS_MAP_CAPACITY, 0);

#[map]
static CGROUP_POLICY_MAP: HashMap<u64, PolicyVerdict> =
    HashMap::with_max_entries(CGROUP_MAP_CAPACITY, 0);

#[map]
static DNS_IP_CACHE_V4: LruHashMap<DekIp4Key, DekDnsCacheValue> =
    LruHashMap::with_max_entries(DEK_DNS_CACHE_MAX_ENTRIES, 0);

#[map]
static METRICS: PerCpuArray<DekMetrics> = PerCpuArray::with_max_entries(1, 0);

#[map]
static EVENTS: RingBuf = RingBuf::with_byte_size(256 * 1024, 0);

#[map]
static DNS_EVENTS: RingBuf = RingBuf::with_byte_size(256 * 1024, 0);

// ------------------------- DNS capture (cgroup_skb) -------------------------

#[cgroup_skb]
pub fn dek_dns_capture(ctx: SkBuffContext) -> i32 {
    // Observe-only: never block. Any parse error is swallowed.
    let _ = try_capture(&ctx);
    1
}

#[inline(always)]
fn load_u8(ctx: &SkBuffContext, off: usize) -> Result<u8, ()> {
    let mut b = [0u8; 1];
    ctx.load_bytes(off, &mut b).map_err(|_| ())?;
    Ok(b[0])
}

#[inline(always)]
fn load_be16(ctx: &SkBuffContext, off: usize) -> Result<u16, ()> {
    let mut b = [0u8; 2];
    ctx.load_bytes(off, &mut b).map_err(|_| ())?;
    Ok(u16::from_be_bytes(b))
}

#[inline(always)]
fn try_capture(ctx: &SkBuffContext) -> Result<(), ()> {
    // cgroup_skb data starts at the IP header (no Ethernet). Read version nibble.
    let first = load_u8(ctx, 0)?;
    let version = first >> 4;

    // Resolve (L4 offset, L4 protocol) for v4/v6.
    let (l4_off, proto) = match version {
        4 => {
            let ihl = (first & 0x0f) as usize * 4; // IHL in 32-bit words -> bytes
            if ihl < 20 {
                return Err(());
            }
            let proto = load_u8(ctx, 9)?; // IPv4 protocol field @ offset 9
            (ihl, proto)
        }
        6 => {
            let next = load_u8(ctx, 6)?; // IPv6 next-header @ offset 6
            (IPV6_HDR_LEN, next)
        }
        _ => return Err(()),
    };

    if proto != IPPROTO_UDP {
        return Err(());
    }

    // UDP header: src(0..2) dst(2..4) len(4..6) csum(6..8)
    let src_port = load_be16(ctx, l4_off)?;
    let dst_port = load_be16(ctx, l4_off + 2)?;
    if src_port != DNS_PORT && dst_port != DNS_PORT {
        return Err(());
    }
    let udp_len = load_be16(ctx, l4_off + 4)? as usize;
    let payload_off = l4_off + UDP_HDR_LEN;
    let payload_len = udp_len.saturating_sub(UDP_HDR_LEN);
    if payload_len == 0 {
        return Err(());
    }

    // Bound the copy length to a power-of-two so the verifier can prove safety.
    // DNS_PAYLOAD_MAX is 512 (2^9); mask to 0..=511.
    let n = payload_len & (DNS_PAYLOAD_MAX - 1);
    if n == 0 {
        return Err(());
    }

    if let Some(mut entry) = DNS_EVENTS.reserve::<DnsCaptureEvent>(0) {
        let p = entry.as_mut_ptr();
        unsafe {
            (*p).cgroup_id = bpf_get_current_cgroup_id();
            (*p).len = n as u16;
            // Bounded copy of the DNS payload into the event buffer.
            let dst = &mut (&mut (*p).data)[..n];
            if ctx.load_bytes(payload_off, dst).is_err() {
                entry.discard(0);
                return Err(());
            }
        }
        entry.submit(0);
    }
    Ok(())
}

// ------------------------- egress guardrail (connect4) -------------------------

#[cgroup_sock_addr(connect4)]
pub fn dek_connect4(ctx: SockAddrContext) -> i32 {
    try_dek_connect4(&ctx).unwrap_or(1) // default ALLOW on error (fail-open)
}

#[inline(always)]
fn try_dek_connect4(ctx: &SockAddrContext) -> Result<i32, ()> {
    let sa = unsafe { &*ctx.sock_addr };
    let dest_ip = u32::from_be(sa.user_ip4);
    let dest_port = u16::from_be(sa.user_port as u16);
    let cgroup_id = unsafe { bpf_get_current_cgroup_id() };
    let pid = (bpf_get_current_pid_tgid() >> 32) as u32;

    let mut verdict = PolicyVerdict { allow: 1, log_event: 0 };

    // 0) DNS TTL Check using LRU MAP
    let dns_key = DekIp4Key {
        ip_be: dest_ip,
        netns_cookie_lo: 0,
        netns_cookie_hi: 0,
    };

    let mut has_dns_context = false;
    let mut is_expired = false;
    let now = unsafe { bpf_ktime_get_ns() };

    if let Some(v) = unsafe { DNS_IP_CACHE_V4.get(&dns_key) } {
        if v.expires_at_ns != 0 && now > v.expires_at_ns {
            is_expired = true;
        } else {
            has_dns_context = true;
        }
    }

    if is_expired {
        unsafe {
            let _ = DNS_IP_CACHE_V4.remove(&dns_key);
        }
    }

    if !has_dns_context {
        // Protected Mode Fallback
        let protected_mode = false; // Could read from another map
        if protected_mode {
            return Ok(0);
        }
    }

    // 1) cgroup-specific policy
    if let Some(v) = unsafe { CGROUP_POLICY_MAP.get(&cgroup_id) } {
        verdict = *v;
    } else {
        // 2) LPM trie (IP/CIDR)
        let key = aya_ebpf::maps::lpm_trie::Key::new(32, dest_ip);
        if let Some(v) = unsafe { VERDICT_MAP.get(&key) } {
            verdict = *v;
        } else if let Some(v) = unsafe { PORTS_MAP.get(&dest_port) } {
            // 3) port policy
            verdict = *v;
        }
    }

    if verdict.log_event != 0 {
        if let Some(mut buf) = EVENTS.reserve::<EgressEvent>(0) {
            let event = EgressEvent {
                pid,
                cgroup_id,
                dest_ip,
                dest_port,
                action_taken: verdict.allow,
            };
            unsafe {
                core::ptr::write_unaligned(buf.as_mut_ptr() as *mut EgressEvent, event);
            }
            buf.submit(0);
        }
    }

    Ok(verdict.allow as i32)
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}

