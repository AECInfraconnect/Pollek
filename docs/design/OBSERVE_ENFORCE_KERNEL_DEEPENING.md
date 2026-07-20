# Observe & Enforce — Kernel-Grade Deepening Design

Status: **Design + first implementation slice.** This PR ships the design, a
device-verification harness, and the first real implementation it calls for —
the userspace policy→map writer (roadmap phase 2, below). Remaining phases land
in follow-up PRs.

This document extends [`POLICY_FIRST_OS_OBSERVE_ENFORCE_WASM.md`](./POLICY_FIRST_OS_OBSERVE_ENFORCE_WASM.md)
and [`policy-enforcement-flows.md`](./policy-enforcement-flows.md). It defines how
Pollek makes its Observe and Enforce planes **genuinely low-level** — using the
same class of OS/kernel techniques that modern antivirus/EDR products use —
while guaranteeing the machine **never crashes**, policy is driven by
hot-reloaded bundles from the portal, and every plane **orchestrates** with the
functions already designed (MCP proxy, guard pipeline, decision engine, cloud
telemetry) and returns **real results**.

---

## 1. Goals & non-goals

**Goals**

1. Real enforcement, not simulation: a policy that says "block" actually stops
   the action at the lowest safe layer available on the device.
2. Real observation: file, network/DNS, process, and tool/MCP activity captured
   from kernel-grade sources, not only log scraping.
3. Cross-platform parity of *intent*: Windows, Linux, macOS each map the same
   policy to the best real mechanism they have.
4. Hot-reload safety: policy bundles pushed from the portal activate without a
   restart and **cannot brick the box** — staged, health-gated, auto-rollback.
5. Orchestration: the kernel plane, the user-mode proxy plane, and the decision
   engine cooperate through one contract and report truthful outcomes.

**Non-goals**

- We do **not** ship an unbounded in-kernel rule engine. Complex matching stays
  in user space by design (see §4, `kernel_guard`).
- We do **not** require a custom Windows kernel driver or a macOS KEXT. We use
  the vendor-supported user-mode / system-extension paths (post-CrowdStrike
  direction, see §2).
- We do **not** claim on-device verification from CI. Kernel attach / packet
  drop is proven by the device harness (§7), not the default pipeline.

---

## 2. Latest-technology research (2025–2026)

The design is anchored to where the platforms are actually going, not to legacy
kernel-driver patterns.

### Linux — eBPF is the mainstream security substrate

- **BPF-LSM / KRSI** (Kernel Runtime Security Instrumentation) is the merged,
  privileged path for MAC + audit via eBPF programs attached to LSM hooks. It
  went from `<5%` of distros shipping it (2021) to `>80%` (2023+), so it is now
  a realistic baseline, not a research toy. It needs `CAP_BPF`/`CAP_SYS_ADMIN`.
- **Landlock** is the *unprivileged* filesystem/network sandbox path (stackable
  LSM, no BPF load privilege). Use it for per-process file confinement where we
  cannot or should not require full privilege.
- **aya 0.13** gives CO-RE ("compile once, run everywhere") pure-Rust eBPF with
  BTF, so a single musl-linked binary loads across kernel versions. Ring buffers
  (Linux ≥ 5.8) are the preferred kernel→user event channel; LPM-trie maps do
  longest-prefix CIDR matching in-kernel.

→ **Decision:** eBPF (cgroup/connect4 + cgroup/skb ring buffer) for
network/DNS observe+enforce; **BPF-LSM** for file/exec enforce where privileged;
**Landlock** as the unprivileged file-sandbox fallback.

### Windows — move *out* of the kernel

- After the July 2024 CrowdStrike outage (a bad kernel-driver content update
  BSOD'd Windows fleets worldwide), Microsoft is previewing a **user-mode
  Windows endpoint security platform** so AV/EDR vendors run **outside the
  kernel**, and is mandating **Safe Deployment Practices** (staged rings, health
  monitoring, gradual rollout, coordinated rollback).
- **WFP** filters can be added from **user mode** via `FwpmEngineOpen0` /
  `FwpmFilterAdd0` at the ALE layers — no kernel callout driver required for
  connection-level allow/deny. **ETW / ETW-TI** provides rich process, image,
  and threat-intelligence telemetry without a kernel agent.

→ **Decision:** Windows enforce = **user-mode WFP** (already real in
`dek-windows-wfp`); Windows observe = **ETW / ETW-TI**. Any future kernel
component stays minimal and behind the watchdog + auto-rollback. This matches
Microsoft's own post-incident direction.

### macOS — System Extensions, not KEXTs

- **EndpointSecurity** (process/file/exec/mount events; can *block* `AUTH`
  events) + **NetworkExtension** content filter, packaged as **System
  Extensions** (KEXTs are deprecated). Requires the ES entitlement + user
  approval; NE needs `com.apple.developer.networking.networkextension`.

→ **Decision:** macOS enforce = **NetworkExtension** content filter (client
already real in `dek-macos-nefilter`) + **EndpointSecurity** for file/exec;
delivered as signed system extensions.

### Cross-cutting — Safe Deployment Practices

The universal lesson from 2024–2025 endpoint incidents: **the update path, not
the sensor, is what bricks machines.** Every hot-reload must be staged,
health-gated, and auto-rollback-capable, and the enforcement update path must
**fail open** (keep the box usable) except where policy explicitly demands
fail-closed.

**Sources:**
[LWN: KRSI](https://lwn.net/Articles/808048/) ·
[LWN: Landlock LSM](https://lwn.net/Articles/803430/) ·
[AccuKnox: eBPF/BPF-LSM runtime security](https://accuknox.com/blog/runtime-security-ebpf-bpf-lsm) ·
[eunomia: eBPF ecosystem 2024–2025](https://eunomia.dev/blog/2025/02/12/ebpf-ecosystem-progress-in-20242025-a-technical-deep-dive/) ·
[aya-rs](https://github.com/aya-rs/aya) ·
[SC Media: Microsoft to move security capabilities outside the kernel](https://www.scworld.com/news/crowdstrike-outage-leads-microsoft-to-plan-more-security-capabilities-outside-of-kernel) ·
[Apple: System Extensions](https://developer.apple.com/system-extensions/) ·
[Apple: EndpointSecurity](https://developer.apple.com/documentation/endpointsecurity)

---

## 3. Current state — what is real vs. what is stubbed

Assessed by reading the crates, not the marketing.

| Component | Crate / file | State | Gap |
|---|---|---|---|
| Linux eBPF programs | `dek-ebpf-prog` | **Real** aya-ebpf 0.13: `dek_dns_capture` (cgroup/skb DNS ring buffer), `dek_connect4` (cgroup/connect4 egress) | — |
| eBPF map keys/events | `dek-ebpf-common` | **Real** shared `#[repr(C)]` types (LPM key, ring events) | — |
| eBPF kernel enforcement | `dek-ebpf-prog` `dek_connect4`/`connect6` | **Real** — reads `RUNTIME_MODE`, `CGROUP_POLICY_MAP`, `VERDICT_MAP` (LPM), `PORTS_MAP` and returns `verdict.allow` (0 = **drops** the connect), with protected-mode fallback + DNS-TTL gating + ring-buffer events | Enforces on whatever the maps contain — but nothing writes bundle policy into them yet (see next row) |
| eBPF load + attach | `dek-ebpfd/lib.rs` `start_ebpfd_supervisor` | **Real** aya — `Ebpf::load` the BTF object, pin policy maps, load+attach `dek_connect4`, drain the DNS ring buffer | — |
| userspace policy→map bridge | `dek-ebpfd/map_updater.rs` | **Real (this PR)** — `apply_update` parses each update into a typed target+verdict and, behind the `kernel-maps` feature on Linux, opens the pinned `VERDICT_MAP`/`PORTS_MAP`/`CGROUP_POLICY_MAP` and writes the real `PolicyVerdict`; validated no-op otherwise | Follow-up: set `RUNTIME_MODE`, real capability probing |
| Windows enforce | `dek-windows-wfp` | **Real** user-mode WFP (`FwpmEngineOpen0`/`FwpmFilterAdd0`, filter add/delete by id) | Companion service packaging/signing |
| macOS enforce | `dek-macos-nefilter` | **Real** NE rule-message client | System-extension host packaging/signing |
| Cross-OS driver | `dek-core/network_loop.rs` | **Real** `NetworkEnforcer` trait + per-OS backends, fail-closed, feature-gated `os-enforcement` | Wire real ebpfd loader into the Linux backend |
| Kernel-safety classifier | `dek-core/kernel_guard.rs` | **Real** — routes only KERNEL-SAFE exact matches to kernel (`MAX_KERNEL_ENTRIES=1024`), everything else to user mode | — |
| Crash safety | `panic_guard`, `watchdog`, `probation`, `supervisor` | **Real** — abort-on-panic hook, liveness heartbeat + sd_notify, health-gated A/B with `.bak` auto-rollback | Wire enforce-plane health into probation signal |
| Warm check | `pep_warm_check.rs` | **Real** MCP-proxy `/health` probe before activation | Add kernel-plane warm check |
| Hot reload | `reload_coordinator.rs` + `dek-bundle-sync` + `dek-policy-syncer` | **Real** generation snapshots → `SyncOutcome` → `network_loop` | Extend to file/process domains |
| Capability report | `dek-core/capabilities.rs` | **Stub** — emits `ebpfd.stub` / `wfp.stub` / `nefilter.stub` | Report real probed capabilities |

**Takeaway:** the architecture is sound and mostly real. The highest-leverage
realness work is (a) the aya userspace loader, (b) real capability probing, and
(c) extending the enforcer trait beyond network to file/process — all behind the
existing safety machinery.

---

## 4. Target architecture

### 4.1 Per-domain enforcer & observer traits

Generalize today's network-only `NetworkEnforcer` into a domain-indexed set so
every control domain maps to the best real backend per OS.

```
                 ┌──────────────────────────────────────────────┐
   portal ─────► │  hot-reload: bundle-sync → policy-syncer      │
   (bundle)      │            → reload_coordinator (generations) │
                 └───────────────┬──────────────────────────────┘
                                 │ activate(snapshot N)  [health-gated]
                 ┌───────────────▼──────────────────────────────┐
                 │  Enforcement Orchestrator (decision engine)   │
                 │  - classify rule → KERNEL-SAFE | USER-MODE    │  kernel_guard
                 │  - dispatch per domain to the right backend   │
                 │  - collect real outcomes → telemetry          │
                 └───┬───────────┬───────────┬───────────┬───────┘
        Network/DNS  │   File     │  Process  │  Tool/MCP │
        ┌────────────▼┐ ┌─────────▼┐ ┌────────▼┐ ┌────────▼─────────┐
 Linux  │cgroup/connect│ │BPF-LSM / │ │BPF-LSM  │ │ mcp-proxy /      │
        │+ skb ringbuf │ │Landlock  │ │exec hook│ │ stdio-wrapper    │
 Win    │user-mode WFP │ │minifilter│ │ETW-TI   │ │ (user mode,      │
 macOS  │NetworkExt    │ │Endpoint  │ │Endpoint │ │  already real)   │
        │              │ │Security  │ │Security │ │                  │
        └──────────────┘ └──────────┘ └─────────┘ └──────────────────┘
```

```rust
/// One trait per control domain; each OS provides a backend or None.
pub trait DomainEnforcer: Send {
    fn domain(&self) -> ControlDomain;              // Network|Dns|File|Process|McpTool
    fn apply(&mut self, rules: &CompiledRuleSet) -> anyhow::Result<ApplyReport>;
    fn fail_open(&mut self) -> anyhow::Result<()>;  // default posture
    fn fail_closed(&mut self) -> anyhow::Result<()>; // StrictDeny only
    fn backend(&self) -> &'static str;
    fn warm_check(&self) -> anyhow::Result<()>;      // health before activation
}
```

`ApplyReport` carries *real* results: how many rules went to kernel vs user
mode, entries installed, and any downgrade (e.g. "requested Enforce, achieved
Observe because BPF-LSM unavailable") — surfaced back to the portal so the UI
never over-claims.

### 4.2 Observe sources (kernel-grade)

| Domain | Linux | Windows | macOS |
|---|---|---|---|
| DNS / network | eBPF cgroup/skb ring buffer (real today) | ETW / WFP audit | NetworkExtension flow |
| File | fanotify / BPF-LSM `file_open` | minifilter / ETW FileIO | EndpointSecurity `open`/`create` |
| Process / exec | tracepoint/BPF-LSM `bprm_check` | ETW-TI process/image | EndpointSecurity `exec` |
| Tool / MCP | mcp-proxy + stdio-wrapper (real today) | same | same |

Observation is **always safe** (never blocks) and feeds the same telemetry
pipeline the cost/activity features already consume.

---

## 5. Crash-safety model (the "never brick the box" contract)

This is the heart of the design and largely **already implemented**; the plan
formalizes and extends it.

1. **Keep complexity out of the kernel** — `kernel_guard` classifies each rule;
   only exact CIDR/port matches under `MAX_KERNEL_ENTRIES` reach the kernel.
   Regex/wildcard/time-window/over-capacity → user-mode plane. The eBPF verifier
   therefore never sees a program/map it can reject or that destabilizes load.
2. **Fail-open by default** — if a backend fails to apply, the domain reverts to
   observe/allow (machine stays usable). **Fail-closed is opt-in** per policy
   (`StrictDeny`) and even then exempts the control-plane mTLS channel so the
   device can still receive a corrective bundle.
3. **Health-gated activation** — a new snapshot is *staged*, then `warm_check()`
   (kernel plane) + `pep_warm_check` (MCP plane) + mTLS-to-cloud must pass for N
   consecutive probes before it becomes active. On failure → keep last-known-good.
4. **A/B binary probation** — `probation.rs` stages a new dek-core binary, and
   only *commits* after health passes; otherwise restores `.bak` and exits so
   the service manager restarts the old binary. No silent bad commit.
5. **Watchdog + panic guard** — `watchdog` heartbeats (sd_notify on Linux);
   `panic_guard` converts any panic into a clean abort (no half-applied kernel
   state). The supervisor owns cancellation so shutdown is orderly.
6. **Staged/canary rollout from the portal** — bundles carry a rollout ring;
   the device honors ring order and reports health so the portal can halt a bad
   rollout fleet-wide (the Safe Deployment Practice from §2).

---

## 6. Hot-reload flow (portal → enforced, safely)

```
portal publishes bundle (signed, versioned, ring-tagged)
      │
      ▼  dek-bundle-sync  (mTLS pull, signature verify, spool to disk)
      ▼  dek-policy-syncer → SyncOutcome
      ▼  reload_coordinator: build RuntimeSnapshot(generation = N+1)
      ▼  STAGE (not yet active)
      ▼  warm_check(kernel) + pep_warm_check(mcp) + cloud mTLS  ── fail ─► keep gen N
      ▼  pass ×N
      ▼  ACTIVATE generation N+1 atomically (router + enforcer apply)
      ▼  DomainEnforcer.apply() → ApplyReport
      ▼  telemetry: real applied/failed/downgraded counts → portal
```

Rollback is just "stay on / revert to generation N": snapshots are immutable and
reference-counted, so activation is a pointer swap with no destructive step.

---

## 7. Verification strategy

Because CI runs unprivileged Linux (no `CAP_BPF`, no Windows/macOS), we split
verification into two tiers, and label every claim by tier.

- **Tier 1 — verified in CI (this repo, every PR):** pure logic — rule→map
  translation, `kernel_guard` classification, decision-engine outcomes, snapshot
  generation/rollback state machine, `ApplyReport` accounting. All unit-tested.
- **Tier 2 — verified on device (the harness, §7 of this doc):** real kernel
  attach + real drop/observe. Run on a privileged host per OS. Gated so it never
  blocks default CI.

### Device test harness (ships in this PR)

- `crates/dek-ebpfd/tests/device_enforcement.rs` — `#[ignore]` by default.
  Detects prerequisites (root, `/sys/fs/bpf`, kernel ≥ 5.8) and **skips with a
  clear reason** when they are absent, so it is safe to invoke anywhere. Where
  the real loader is present it asserts a genuine effect; until then it exercises
  the Tier-1 translation and marks the exact assertion points for the loader PR.
- `docs/runbooks/ENFORCE_DEVICE_VERIFICATION.md` — step-by-step per-OS
  verification (Linux eBPF drop, Windows WFP filter, macOS NE filter).
- `.github/workflows/device-enforcement.yml` — **manual** `workflow_dispatch`
  privileged Linux job. Opt-in only; the normal PR pipeline is unaffected.

---

## 8. Phased roadmap (follow-up PRs)

1. **This PR** — design + device-verification harness + runbook (no behavior
   change; default CI green).
2. **Linux eBPF enforcement made real** — the kernel program already drops on
   map contents and the load/attach path already works.
   - **Done in this PR:** `map_updater` now parses each compiled update into a
     typed `MapTarget` + `ParsedVerdict` and, behind the `kernel-maps` feature
     on Linux, opens the pinned `VERDICT_MAP`/`PORTS_MAP`/`CGROUP_POLICY_MAP` via
     aya and writes/deletes the real `PolicyVerdict` entry (host-order LPM key
     matching the kernel). Off-feature/off-Linux it is a validated no-op
     (fail-open). Tier-1 unit tests cover CIDR/port/cgroup translation,
     verdict parsing, name aliasing, and the validate/generation guards.
   - **Follow-up:** set `RUNTIME_MODE` for protected-mode, real capability
     probing to replace the `*.stub` strings, and the Tier-2 harness drop
     assertion driving a real connection.
3. **Domain generalization** — `DomainEnforcer` trait + orchestrator; add
   Linux file (Landlock/BPF-LSM) and process (exec hook) backends; `ApplyReport`
   downgrade surfacing to the portal.
4. **Windows depth** — ETW/ETW-TI observe; WFP `ApplyReport` + warm check;
   companion-service packaging with Safe Deployment ring support.
5. **macOS depth** — EndpointSecurity file/exec observe+block; NE `ApplyReport`;
   system-extension packaging/signing.
6. **Canary/rollout orchestration** — ring-aware bundle activation end-to-end
   with portal-driven halt on unhealthy fleet.

Each phase keeps the §5 safety contract and reports real, tier-labeled results.
