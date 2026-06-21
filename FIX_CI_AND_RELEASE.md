# แก้ CI Failing + No Releases Found ให้เป็นปกติ

repo `AECInfraconnect/AntiG_Pollen_DEK` (main, v25) — README badge แดง 2 ตัว:
`CI failing` + `release no releases found`

---

## สรุปสาเหตุ (ตรวจจาก source จริง)

| Badge | สาเหตุ |
|---|---|
| 🔴 **CI failing** | (1) **stray files ที่ root** — `scratch_rcgen.rs`, `test-sig.rs`, `test_rcgen.rs`, `bootstrap.json`, `test_bootstrap.json` หลุดเข้า repo → fmt/clippy เจอไฟล์ .rs นอก crate; (2) อาจมี fmt/clippy warning จาก code ใหม่ |
| 🔴 **no releases found** | **ยังไม่ได้ push tag `v1.0.0-beta.1`** — release.yml trigger เฉพาะ `push: tags: v*.*.*` → ไม่มี tag = ไม่มี release (โค้ด pipeline พร้อมแล้ว แค่ยังไม่ tag) |

> badge release อ่านจาก GitHub Releases API — "no releases found" = ยังไม่เคยสร้าง release เลย
> badge CI อ่านจาก workflow run ล่าสุดบน main — failing = ci.yml มี step ที่ exit non-zero

---

## PART 1 — แก้ CI Failing

### 1.1 ลบ stray files ที่ root (สาเหตุหลัก)
ไฟล์ scratch เหล่านี้หลุดเข้า repo (จาก dev/debug) — ไม่ใช่ crate, ไม่อยู่ใน workspace members:
```bash
git rm scratch_rcgen.rs test-sig.rs test_rcgen.rs bootstrap.json test_bootstrap.json
```
> `test-sig.rs`/`test_rcgen.rs`/`scratch_rcgen.rs` = .rs ลอยที่ root → `cargo fmt --all --check` และ clippy
> อาจ error (ไฟล์ไม่อยู่ใน crate / ไม่ผ่าน format) หรือทำ build สับสน
> `bootstrap.json`/`test_bootstrap.json` = test data ลอย → ควรอยู่ใน `crates/*/tests/fixtures/` ถ้าใช้จริง

ป้องกันซ้ำใน `.gitignore`:
```gitignore
/scratch_*.rs
/test-*.rs
/test_*.rs
/test_*.json
/bootstrap.json
```
> ระวัง: ถ้าบาง test ต้องใช้ `bootstrap.json` ให้ย้ายเข้า `tests/fixtures/` + อ้าง path นั้น ไม่ใช่ลบเฉย ๆ
> ตรวจก่อน: `grep -rn "test_bootstrap\|test_rcgen\|test-sig" crates/` → ถ้ามี test อ้างถึง ให้ย้ายแทนลบ

### 1.2 รัน CI steps แบบเดียวกับ workflow (reproduce local)
ci.yml รัน 4 step ตามลำดับ — รันเองให้ผ่านทั้งหมดก่อน push:
```bash
# 1) fmt (step แรกที่มักทำ fail)
cargo fmt --all -- --check
# ถ้า fail -> cargo fmt --all  แล้ว commit

# 2) clippy -D warnings (ทุก warning = error)
cargo clippy --workspace --exclude dek-ebpf-prog --exclude dek-ebpfd \
  --all-targets --all-features -- -D warnings

# 3) test
cargo test --workspace --exclude dek-ebpf-prog --exclude dek-ebpfd --all-features

# 4) build release
cargo build --workspace --exclude dek-ebpf-prog --exclude dek-ebpfd --release
```

### 1.3 แก้ clippy/fmt ที่เหลือ (ถ้ามี)
- code ใหม่ (engine_selector/kernel_guard/jwt_svid/trust_bundle) อาจมี clippy lint → แก้ตามที่ clippy บอก
- `--all-features` เปิดทุก feature → adapter ทั้งหมด compile → ต้องไม่มี warning ในทุก path
- ดู eBPF/WASM job: `wasm_build` exclude dek-core + ebpf — ถ้า crate ใหม่ไม่ build บน wasm32 ต้อง exclude เพิ่ม

### 1.4 ตรวจ test ที่อาจ fail บน CI (ไม่ใช่ local)
- e2e ที่ต้อง binary → ต้อง `cargo build` ก่อน (มี harness) — ถ้า unit test ปนกับ e2e ใน `cargo test` ปกติ อาจ fail เพราะไม่มี service
- ปกติ `#[ignore]` กัน e2e ออกจาก `cargo test` ปกติแล้ว → ยืนยัน local_e2e/soak/matrix มี `#[ignore]`
- timing flaky → ใช้ C3 (poll_until) ที่ทำไว้

### Acceptance Part 1
- [ ] ไม่มี stray .rs/.json ที่ root; `.gitignore` กันแล้ว
- [ ] `cargo fmt --all --check` ผ่าน
- [ ] `cargo clippy ... -D warnings` ผ่าน (ทุก OS)
- [ ] `cargo test --workspace` ผ่าน (unit; e2e เป็น ignored)
- [ ] CI badge → passing

---

## PART 2 — แก้ No Releases Found

### 2.1 สาเหตุ: ยังไม่ push tag
release.yml พร้อมแล้ว (build 3 OS + cosign + SBOM + `action-gh-release`) แต่ trigger คือ:
```yaml
on:
  push:
    tags: ['v*.*.*', 'v*.*.*-*']   # ต้อง push tag ถึงจะรัน
```
→ **ยังไม่มี tag ใด ๆ** = release job ไม่เคยรัน = no releases

### 2.2 ขั้นตอน tag + release (ทำหลัง CI เขียว)
```bash
# 1) ยืนยัน CI บน main เขียวก่อน (Part 1 เสร็จ)
# 2) ยืนยัน version ตรง
grep '^version' Cargo.toml   # ควร 1.0.0-beta.1

# 3) สร้าง + push tag (annotated)
git tag -a v1.0.0-beta.1 -m "Pollen DEK v1.0.0-beta.1 — first public beta"
git push origin v1.0.0-beta.1
```
→ trigger release.yml: build 3 OS → sign (cosign) → SBOM → `action-gh-release` สร้าง Release page (prerelease=true เพราะ tag มี `-`)

### 2.3 ตรวจ release.yml ก่อน tag (กัน job fail กลางทาง)
release.yml ใช้ build matrix เดียวกับ CI → ถ้า CI เขียว release build ก็ควรผ่าน. ตรวจเพิ่ม:
- [ ] `permissions: contents: write` ใน release job (ต้องมีเพื่อสร้าง release) — ยืนยันมี
- [ ] cosign step ใช้ OIDC (`id-token: write` permission)
- [ ] secrets ที่ต้องใช้ (ถ้ามี codesign/notarize) ตั้งไว้ครบ — ถ้ายังไม่มี cert ให้ใช้ cosign keyless อย่างเดียว (beta)
- [ ] `release-gate.yml` (trigger บน tag เดียวกัน) ต้องเขียว — ถ้า gate fail อาจบล็อก; ตรวจว่า gate ไม่ block publish job หรือรัน parallel

### 2.4 ถ้าอยากทดสอบก่อน tag จริง
```bash
# workflow_dispatch (manual) — release.yml มี input version
# ไปที่ Actions -> Release -> Run workflow -> version: v1.0.0-beta.1-rc1
# หรือ tag ทดสอบ:
git tag v0.0.1-test && git push origin v0.0.1-test   # ดูว่า pipeline ครบ
# ลบ test release + tag หลังตรวจเสร็จ
```
> มี `release-dry-run.yml` (trigger บน PR) — รันบน PR เพื่อ validate ก่อน merge ด้วย

### Acceptance Part 2
- [ ] CI เขียวบน main ก่อน
- [ ] push tag `v1.0.0-beta.1` → release.yml รันสำเร็จ
- [ ] Release page ปรากฏ + binary 3 OS + SHA256SUMS + cosign .sig/.pem + SBOM
- [ ] release badge → `v1.0.0-beta.1`
- [ ] download + verify (sha256 + cosign) ได้จริง

---

## ลำดับการแก้ (สำคัญ — ต้องตามลำดับ)
1. **Part 1 ก่อน** (ลบ stray files + fmt/clippy/test เขียว) → push main → CI badge เขียว
2. **แล้วค่อย Part 2** (tag v1.0.0-beta.1) → release badge เขียว
> ห้าม tag ก่อน CI เขียว — release build จะ fail ด้วยสาเหตุเดียวกัน

## สรุป
| Badge | แก้ | ไฟล์/คำสั่ง |
|---|---|---|
| CI failing | ลบ stray root files + fmt/clippy/test ผ่าน | `git rm scratch_rcgen.rs test-sig.rs test_rcgen.rs *.json` + fmt/clippy |
| no releases | push tag (pipeline พร้อมแล้ว) | `git tag -a v1.0.0-beta.1 && git push origin v1.0.0-beta.1` |

ทั้งสองไม่ใช่ bug ในระบบ — **CI = scratch files หลุดเข้า repo, release = ยังไม่ tag** แก้ตรงจุดแล้ว badge เขียวทั้งคู่
