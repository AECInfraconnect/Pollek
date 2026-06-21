# C4 — Code Hygiene Polish (clippy/fmt/thiserror/dead-code)

## ผลตรวจ (v24) — scope เล็ก, โครงสร้างดีอยู่แล้ว
- anyhow ใน public API: แค่ 2 crate (`dek-openfga` 1 fn, `dek-policy-syncer` 1 fn) — ที่เหลือ clean
- `#[allow(dead_code)]` 11 จุด: ส่วนใหญ่ `mock-cloud` (test infra, ยอมรับได้) + `dek-core` (ต้องเช็คทีละตัว)
- `dek-auth` ถูกใช้จริง (mcp-proxy/activation) — **ไม่ใช่ unused**

---

## C4.1 — fmt + clippy ทั้ง workspace
```bash
cargo fmt --all
cargo clippy --workspace --all-targets --exclude dek-ebpf-prog --exclude dek-ebpfd -- -D warnings
```
workspace มี `unwrap_used=deny`/`expect_used=deny` แล้ว → ควร clean; แก้ warning ที่เหลือ (ส่วนใหญ่ needless_clone/redundant)

## C4.2 — dead_code: ลบจริง หรือ ใช้จริง (ไม่ silence)
ตรวจทีละตัวใน `dek-core`:
| ไฟล์ | การกระทำ |
|---|---|
| `updater.rs` (ถูกอ้างจาก bundle_loop) | ถ้าใช้ → ลบ `#[allow(dead_code)]`; ถ้าไม่ → ลบ module |
| `probation.rs` (ใช้ใน supervisor) | ใช้จริง → ลบ allow, เปิด field ที่ dead ออกถ้าไม่ใช้ |
| `bundle_loop.rs::spawn_bundle_sync_task` | ถ้า network_loop/syncer แทนแล้ว → **ลบทั้ง module** (legacy path) |
| `metrics_push.rs` | ถ้า telemetry push ใช้แทน → ลบ; ถ้า roadmap → ย้ายไป feature flag |
> หลัก: `#[allow(dead_code)]` = หนี้ — ทุกตัวต้องตัดสิน "ใช้/ลบ" ไม่ปล่อย silence
> mock-cloud dead_code = test helper → ยอมรับได้ แต่ใส่ comment เหตุผล หรือ `#[cfg(test)]`

## C4.3 — library error types: anyhow → thiserror (2 จุด)
library ควร return error type ของตัวเอง (caller match ได้); anyhow เก็บไว้ที่ binary/app boundary

### dek-openfga (1 pub fn)
```rust
#[derive(Debug, thiserror::Error)]
pub enum OpenFgaError {
    #[error("connection failed: {0}")] Connection(String),
    #[error("invalid model: {0}")] Model(String),
    #[error("evaluation failed: {0}")] Eval(String),
}
// pub fn ... -> Result<_, OpenFgaError>  (แทน anyhow::Result)
```
### dek-policy-syncer (1 pub fn)
```rust
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("fetch failed: {0}")] Fetch(String),
    #[error("verify failed: {0}")] Verify(String),
    #[error("activation failed: {0}")] Activation(String),
}
```
> ภายใน (private fn) ใช้ anyhow ได้; เฉพาะ **public API** ที่ caller ต้อง match ควร typed
> ถ้า 1 fn นั้นเป็น internal-ish (เรียกจาก binary เดียว) อาจคงไว้ — ใช้ดุลพินิจ ไม่ over-engineer

## C4.4 — unused deps / crates
```bash
cargo install cargo-machete --locked && cargo machete    # unused deps ใน Cargo.toml
cargo install cargo-udeps --locked && cargo +nightly udeps  # unused (ละเอียดกว่า, ต้อง nightly)
```
- ลบ dep ที่ไม่ถูก import
- `dummy_policy` crate: ถ้าเป็น test fixture → ย้ายไป `tests/fixtures` หรือ `#[cfg(test)]`; ถ้าไม่ใช้ → ลบ
- `dek-auth`: **ใช้จริง** (mcp-proxy/activation) → คงไว้

## C4.5 — doc comments บน public API
- ทุก `pub trait`/`pub struct`/`pub fn` ที่ข้าม crate มี `///` (โดยเฉพาะ `dek-control-plane-api`, `dek-pdp-sdk`, `dek-policy-runtime`)
- `#![warn(missing_docs)]` ใน SDK crates (dek-pdp-sdk/dek-plugin-sdk) เพื่อบังคับ doc

## Acceptance C4
- [ ] `cargo fmt --check` clean
- [ ] `cargo clippy -- -D warnings` clean (ทั้ง workspace ex-ebpf)
- [ ] `#[allow(dead_code)]` เหลือเฉพาะที่ justify ได้ (mock test infra) + มี comment
- [ ] public library API ใน dek-openfga/dek-policy-syncer ใช้ typed error (หรือ justify คงไว้)
- [ ] ไม่มี unused dep (machete clean); dummy_policy ตัดสินแล้ว
- [ ] SDK crates มี doc บน public API

## Guardrails
- เปลี่ยนเชิงคุณภาพเท่านั้น — error type เปลี่ยน signature ภายนอก: **bump version + update caller** (ไม่เปลี่ยน behavior)
- ลบ dead code ต้องมั่นใจไม่มี caller (grep ทั้ง workspace + tests) ก่อนลบ
- ไม่ลบ `dek-auth` (ใช้จริง); ไม่ silence warning ด้วย allow โดยไม่ตัดสิน
