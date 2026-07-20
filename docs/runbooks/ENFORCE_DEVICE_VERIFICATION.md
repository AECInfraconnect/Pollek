# Runbook — Device verification of the Observe/Enforce plane

Tier-2 verification (real kernel attach / drop / observe) for the low-level
enforcement plane. See the design in
[`docs/design/OBSERVE_ENFORCE_KERNEL_DEEPENING.md`](../design/OBSERVE_ENFORCE_KERNEL_DEEPENING.md).

CI runs unprivileged Linux and only covers Tier-1 (pure logic). The steps below
prove the real kernel behavior on a privileged host, per OS. None of this is
required for a normal PR to pass.

> Safety: run on a **disposable test machine or VM**, never a production box.
> The enforce plane is fail-open by default, but you are exercising kernel
> attach paths — treat it like any EDR bring-up.

---

## Linux — eBPF load + attach (+ drop, once the loader PR lands)

Prerequisites:

- Root (or `CAP_BPF` + `CAP_SYS_ADMIN`), kernel **>= 5.8**, `bpf` filesystem
  mounted at `/sys/fs/bpf`:

  ```bash
  sudo mount -t bpf bpf /sys/fs/bpf   # if not already mounted
  uname -r                             # expect >= 5.8
  ```

- The eBPF object built for the bpf target (produced by `dek-ebpf-prog` via
  `aya-build` in the crate's `build.rs`). A normal host build embeds a
  placeholder; build the real object first:

  ```bash
  rustup target add bpfel-unknown-none
  cargo build -p dek-ebpf-prog        # emits the BTF object into OUT_DIR
  ```

Run the harness:

```bash
sudo -E POLLEK_DEVICE_ENFORCE_TEST=1 \
    cargo test -p dek-ebpfd --test device_enforcement -- --ignored --nocapture
```

Expected today: `PASS: eBPF object loaded and programs attached to
/sys/fs/cgroup/pollek-device-test` — this proves the object loads and
`dek_connect4` / `dek_dns_capture` attach in-kernel. If you see
`SKIP: ... POLLEK_DEVICE_ENFORCE_TEST!=1` or a prerequisite SKIP, fix that first.

Real-drop assertion (added by the Linux-enforcement PR): after that PR wires the
verdict maps + flips `dek_connect4` to drop, the harness will additionally drive
a connection to a denied CIDR through the test cgroup and assert it is blocked.
Manual check in the meantime:

```bash
# attach a shell to the supervised cgroup, then verify egress behavior
echo $$ | sudo tee /sys/fs/cgroup/pollek-device-test/cgroup.procs
curl -v --max-time 5 https://<denied-host>/    # expect blocked once drop lands
```

DNS observation check (works today): with the supervisor running, resolve a
name from inside the cgroup and confirm a `DnsObservation` reaches telemetry
(the ring buffer → hickory parse path).

---

## Windows — user-mode WFP filter

Prerequisites: Windows 10/11, an elevated (Administrator) context. No custom
kernel driver is required — filters are added via user-mode FWPM
(`FwpmEngineOpen0` / `FwpmFilterAdd0`), as implemented in `dek-windows-wfp`.

Verify:

1. Build the enforcement-enabled dek-core: `cargo build -p dek-core --features os-enforcement` (on Windows).
2. Apply a deny rule for a test destination via the normal bundle path (or the
   crate's test entry), then confirm the connection is blocked:

   ```powershell
   Test-NetConnection <denied-host> -Port 443   # expect failure while active
   netsh wfp show filters                        # the Pollek filter is listed
   ```

3. Remove the rule and confirm connectivity returns (filters are deleted by id).

Observe (ETW/ETW-TI) is validated once the Windows observe PR lands; until then
network audit is available via WFP audit events.

---

## macOS — NetworkExtension content filter

Prerequisites: macOS 11+ (Big Sur or later), the app + system extension signed
with the `com.apple.developer.networking.networkextension` entitlement, and the
extension approved by the user (System Settings → Privacy & Security).

Verify:

1. Install/approve the system extension hosting `dek-macos-nefilter`.
2. Apply a deny rule via the bundle path; confirm the flow is blocked and the
   extension logs the decision:

   ```bash
   log stream --predicate 'subsystem CONTAINS "pollek"' --info
   curl -v --max-time 5 https://<denied-host>/   # expect blocked while active
   ```

3. Remove the rule and confirm the flow is allowed again.

EndpointSecurity file/exec verification is added by the macOS observe PR.

---

## CI: opt-in privileged job

`.github/workflows/device-enforcement.yml` runs the Linux harness on a
**manual** `workflow_dispatch` trigger only. It never runs on push/PR, so the
default pipeline stays green. Trigger it from the Actions tab when a privileged
runner is available.
