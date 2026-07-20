//! Tier-2 device verification harness for the eBPF enforce/observe plane.
//!
//! See `docs/design/OBSERVE_ENFORCE_KERNEL_DEEPENING.md` §7 and
//! `docs/runbooks/ENFORCE_DEVICE_VERIFICATION.md`.
//!
//! These tests are `#[ignore]` by default: they need a privileged Linux host
//! (root, a mounted bpf filesystem, kernel >= 5.8) that CI does not provide, so
//! the normal `cargo test` run never executes them. Run them on a real device:
//!
//!     sudo -E POLLEK_DEVICE_ENFORCE_TEST=1 \
//!         cargo test -p dek-ebpfd --test device_enforcement -- --ignored --nocapture
//!
//! When prerequisites are missing they print a clear SKIP reason and pass, so
//! the harness is safe to invoke anywhere without producing false failures.

use dek_ebpfd::map_updater::MapUpdater;
use dek_ebpfd::probe_available;

/// Outcome of the environment probe, so each test can decide skip vs. run.
struct DeviceEnv {
    os_linux: bool,
    is_root: bool,
    bpffs_mounted: bool,
    kernel_ok: bool,
    opted_in: bool,
}

impl DeviceEnv {
    fn detect() -> Self {
        let os_linux = cfg!(target_os = "linux");
        let is_root = current_euid() == 0;
        let bpffs_mounted = std::path::Path::new("/sys/fs/bpf").exists();
        let kernel_ok = kernel_at_least(5, 8);
        let opted_in = std::env::var("POLLEK_DEVICE_ENFORCE_TEST").as_deref() == Ok("1");
        Self {
            os_linux,
            is_root,
            bpffs_mounted,
            kernel_ok,
            opted_in,
        }
    }

    /// Returns Some(reason) when the real kernel path cannot run here.
    fn skip_reason(&self) -> Option<String> {
        if !self.opted_in {
            return Some(
                "POLLEK_DEVICE_ENFORCE_TEST!=1 (set it to opt in to the real kernel path)".into(),
            );
        }
        if !self.os_linux {
            return Some("not Linux (eBPF plane is Linux-only)".into());
        }
        if !self.is_root {
            return Some("not root / no CAP_BPF (eBPF load needs privilege)".into());
        }
        if !self.bpffs_mounted {
            return Some("/sys/fs/bpf is not mounted (mount -t bpf bpf /sys/fs/bpf)".into());
        }
        if !self.kernel_ok {
            return Some("kernel < 5.8 (ring buffer / modern BPF maps unavailable)".into());
        }
        None
    }
}

fn current_euid() -> u32 {
    // Read from /proc to avoid pulling libc into a dev-only harness.
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status.lines().find_map(|line| {
                line.strip_prefix("Uid:").and_then(|rest| {
                    // Uid: real effective saved fs  -> take the effective (2nd) field.
                    rest.split_whitespace().nth(1)?.parse::<u32>().ok()
                })
            })
        })
        .unwrap_or(u32::MAX)
}

fn kernel_at_least(major: u32, minor: u32) -> bool {
    let release = std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default();
    let mut parts = release.split(|c: char| c == '.' || c == '-');
    let maj = parts
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    let min = parts
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    maj > major || (maj == major && min >= minor)
}

/// Tier-1: the map-update translation logic runs on any host (no privilege).
/// This is the pure logic that the real pinned-map write (loader PR) will feed.
#[test]
fn tier1_map_update_translation_is_wellformed() {
    use dek_domain_schema::ebpf::{EbpfMapUpdate, UpdateSource};

    let mut updater = MapUpdater::new("tenant-x".into(), "device-y".into(), 0);
    let update = EbpfMapUpdate {
        schema_version: "1.0".into(),
        map_name: "egress_lpm_v4".into(),
        operation: "insert".into(),
        source: UpdateSource::Bundle,
        tenant_id: "tenant-x".into(),
        device_id: "device-y".into(),
        generation: 1,
        key: serde_json::json!({ "cidr": "10.0.0.0/8" }),
        value: serde_json::json!({ "allow": 0, "log_event": 1 }),
        signature: None,
    };
    // validate_update must accept a well-formed, correctly-scoped update.
    updater
        .validate_update(&update)
        .expect("well-formed update should validate");
    // apply_update must accept it and advance the tracked generation.
    updater
        .apply_update(update)
        .expect("well-formed update should apply");
}

/// Tier-2: load + attach the real eBPF object in-kernel and assert a live
/// handle. Proves the object loads and programs attach on this device. The
/// verdict-map *drop* assertion is added by the Linux-enforcement PR (see the
/// roadmap in the design doc); this test marks that point explicitly.
#[test]
#[ignore = "requires a privileged Linux host; run with --ignored on a device"]
fn tier2_ebpf_loads_and_attaches() {
    let env = DeviceEnv::detect();
    if let Some(reason) = env.skip_reason() {
        eprintln!("SKIP tier2_ebpf_loads_and_attaches: {reason}");
        return;
    }

    assert!(
        probe_available(),
        "probe_available() must be true on a supported Linux host"
    );

    #[cfg(target_os = "linux")]
    {
        // A dedicated test cgroup keeps the harness from touching real workloads.
        let cgroup = "/sys/fs/cgroup/pollek-device-test";
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let result = rt.block_on(dek_ebpfd::start_ebpfd_supervisor(cgroup, None, None));
        match result {
            Ok(handle) => {
                eprintln!("PASS: eBPF object loaded and programs attached to {cgroup}");
                // Dropping the handle detaches cleanly (see EbpfHandle::drop).
                drop(handle);
                // NEXT (Linux-enforcement PR): drive a connection to a denied
                // CIDR through this cgroup and assert it is dropped once
                // dek_connect4 reads the verdict maps.
            }
            Err(err) => {
                let msg = err.to_string();
                if msg.contains("placeholder is empty") {
                    eprintln!(
                        "SKIP: BPF object not built. Build dek-ebpf-prog for the bpf target \
                         first, then re-run. ({msg})"
                    );
                    return;
                }
                panic!("eBPF load/attach failed on a privileged host: {msg}");
            }
        }
    }
}
