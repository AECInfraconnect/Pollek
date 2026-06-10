# Feature 3 Wiring — Adaptive engine select + kernel complexity guard

## ไฟล์ (มาด้วย)
- `dek-policy-router/src/engine_selector.rs` — `DecisionKind`/`infer_kind`/`select`/`resolve` + tests
- `dek-core/src/kernel_guard.rs` — `classify_destination`/`partition_rules`/`kernel_subset` + tests

---

## 3.1 — Engine selector เข้า router

### dek-policy-router/src/lib.rs: เพิ่ม module + evaluator_ids
```rust
mod engine_selector;
pub use engine_selector::{DecisionKind, EngineSelector};

impl PolicyRouter {
    /// ids ของ evaluator ที่ register จริงใน build นี้ (feature-gated adapters)
    pub fn evaluator_ids(&self) -> Vec<String> {
        self.evaluators.keys().cloned().collect()
    }
}
```

### evaluate(): เมื่อ route ไม่ระบุ PDP → auto-select (จุด `to_evaluate`)
แทน/เสริม block สร้าง `to_evaluate` เดิม:
```rust
let mut to_evaluate = route.pdp_required.clone();
for cond in &route.pdp_conditional {
    if payload.get(&cond.required_payload_key).is_some() || cond.required_payload_key == "*" {
        to_evaluate.push(cond.evaluator.clone());
    }
}
if !route.pdp_pool.is_empty() {
    if let Some(pdp) = self.select_pdp_from_pool(&route.pdp_pool, &route.failover_strategy) {
        to_evaluate.push(pdp);
    }
}
// AUTO-SELECT: ไม่มี PDP ระบุเลย -> เลือก engine ตาม decision kind
if to_evaluate.is_empty() {
    let available = self.evaluator_ids();
    match EngineSelector::resolve(method, &payload, &available) {
        Some(engine) => {
            tracing::info!("auto-selected engine '{}' (kind inferred from request)", engine);
            to_evaluate.push(engine);
        }
        None => {
            // fail-closed: ไม่มี engine ที่เหมาะ + build ไม่มี -> deny
            return Ok(PolicyDecision {
                evaluator_id: "router_autoselect".into(), evaluator_type: "router".into(),
                required: true, status: "success".into(), decision: "deny".into(), allow: false,
                reason: "no suitable policy engine available for request".into(),
                effects: serde_json::json!({}), obligations: vec![],
                metadata: serde_json::json!({ "auto_select": "none_available" }),
            });
        }
    }
}
```
> `method` + `payload` มีใน scope evaluate อยู่แล้ว
> เลือกเฉพาะ engine ที่ register (build ด้วย feature flag) — ไม่เลือกตัวที่ไม่ได้ compile
> decision metadata เพิ่ม `selected_engine` (audit เห็นว่าเลือกอะไร):
```rust
combined_decision.metadata = serde_json::json!({
    "matched_route": route.id,
    "selected_engines": to_evaluate,
    "auto_selected": route.pdp_required.is_empty() && route.pdp_pool.is_empty(),
});
```

### Acceptance 3.1
- [ ] route ไม่ระบุ PDP + payload เป็น relation → เลือก `openfga` (ถ้า build มี)
- [ ] network request (method `net.*` / มี `destination`) → เลือก `ebpf`
- [ ] complex (`rego_query`) → `opa_wasm`; default → `cedar`
- [ ] build ที่มีแค่ Cedar → ทุก kind ตกมา `cedar` (หรือ deny ถ้าไม่เหมาะเลย); ไม่เลือก engine ที่ไม่ได้ compile
- [ ] ไม่มี engine เหมาะ → deny (fail-closed)
- [ ] engine_selector tests ผ่าน

---

## 3.2 — Kernel complexity guard เข้า network_loop

### dek-core/src/main.rs (หรือ lib): เพิ่ม module
```rust
mod kernel_guard;
```

### network_loop: ก่อน apply rules ลง kernel → partition
ตรงที่เดิม apply `CompiledNetworkRules` ลง backend (eBPF/WFP):
```rust
use crate::kernel_guard::{kernel_subset, partition_rules};

// เดิม: backend.apply_rules(&compiled)?;
// ใหม่: แยก kernel-safe vs user-mode ก่อน bind
let (kernel_rules, part) = kernel_subset(&compiled);
tracing::info!(
    "network rules: {} kernel-safe, {} user-mode ({} overflow) — complexity-guarded",
    part.kernel.len(), part.user_mode.len(), part.overflow_to_user
);

// 1) kernel plane: เฉพาะ rule ง่าย (eBPF/WFP) — กัน verifier reject / overload / crash
if !kernel_rules.conditions.destinations.is_empty() {
    if let Err(e) = backend.apply_rules(&kernel_rules) {
        // kernel โหลดไม่ได้ -> fail-closed: เข้า block-all ที่ kernel + ปล่อย user-mode คุม
        tracing::error!("kernel apply failed: {e}; engaging fail_closed");
        backend.fail_closed()?;
    }
}

// 2) user-mode plane: complex rules (regex/conditional/time-window/overflow)
//    เข้า proxy/PDP (Cedar/OPA) — bind ผ่าน user-mode enforcer ที่มีอยู่
if !part.user_mode.is_empty() {
    // ส่ง part.user_mode ให้ user-mode enforcer / สร้าง route เข้า PDP network policy
    user_mode_enforce(&part.user_mode, &compiled)?;
    // (ถ้ายังไม่มี user_mode_enforce: log + เก็บใน proxy match list สำหรับ egress check)
}
```
> **กัน system crash (เป้าหลัก):** kernel รับเฉพาะ exact CIDR/port/domain + จำกัด MAX_KERNEL_ENTRIES=1024;
> regex/wildcard/conditional/time-window/overflow → user-mode plane
> eBPF verifier มี limit → guard กันไว้ก่อน load; ถ้า kernel ยัง apply ไม่ได้ → fail_closed (block-all ที่ kernel)

### honesty: รายงาน plane ใน enforcement state / capability
```rust
// network plane status: ระบุว่า rule ไหน enforce ที่ kernel vs user-mode
metrics::gauge!("dek_network_rules_kernel").set(part.kernel.len() as f64);
metrics::gauge!("dek_network_rules_usermode").set(part.user_mode.len() as f64);
```

### Acceptance 3.2
- [ ] CIDR/port/exact-domain → kernel plane (eBPF apply)
- [ ] wildcard domain/regex/conditional/time-window → user-mode plane (ไม่ลง kernel)
- [ ] destinations เกิน 1024 → overflow → user-mode (kernel ไม่เต็ม/ไม่ crash)
- [ ] kernel apply fail → fail_closed (block-all) — ไม่ปล่อยผ่าน
- [ ] kernel_guard tests ผ่าน (classify/partition/cap/subset)

---

## Guardrails
- เลือกเฉพาะ engine ที่ build มี (feature flags) — ไม่ fabricate
- fail-closed: ไม่มี engine เหมาะ → deny; kernel apply fail → block-all
- kernel complexity guard = กัน complex policy ลง kernel (verifier reject/crash) → 2-plane (kernel เร็ว+การันตี / user-mode ยืดหยุ่น)
- coverage ครบ: rule ที่ไม่ลง kernel ไม่ถูกทิ้ง — ไป user-mode (ไม่ silent drop)
- decision/metric audit เห็น engine + plane

## สรุป Feature 3
| Task | ไฟล์ | สถานะ |
|---|---|---|
| 3.1 engine selector | `dek-policy-router/src/engine_selector.rs` | **โค้ดจริง + tests** |
| 3.1 wire router | `dek-policy-router/src/lib.rs` | wiring |
| 3.2 kernel guard | `dek-core/src/kernel_guard.rs` | **โค้ดจริง + tests** |
| 3.2 wire network_loop | `dek-core/src/network_loop.rs` | wiring |
