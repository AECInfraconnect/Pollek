//! Pollek contract adapter — a hot-reloadable WASM component that migrates a
//! policy bundle authored for one contract generation into the shape the
//! running DEK expects. This is the version-skew bridge: a fleet of DEKs at
//! different versions can share Cloud-authored bundles because each DEK runs
//! this adapter (itself hot-reloadable) over an incoming bundle before
//! activating it.
//!
//! Core-wasm ABI expected by `dek-wasm-host`:
//!   - exported `memory`
//!   - `alloc(size) -> ptr`, `dealloc(ptr, len)`
//!   - `pollek_plugin_reset() -> i32`
//!   - `pollek_plugin_decide(ptr, len) -> i64`  (packed `(out_ptr << 32) | out_len`)
//!
//! Input JSON:  `{ "bundle": <bundle>, "to_contract": "2026.06.29" }`
//! Output JSON: `{ "adapted": bool, "from_contract": str, "to_contract": str,
//!                 "changes": [str], "bundle": <migrated bundle> }`

use serde_json::{json, Value};

#[no_mangle]
pub extern "C" fn alloc(size: i32) -> i32 {
    let mut buf = Vec::<u8>::with_capacity(size.max(0) as usize);
    let ptr = buf.as_mut_ptr();
    core::mem::forget(buf);
    ptr as i32
}

#[no_mangle]
pub extern "C" fn dealloc(ptr: i32, len: i32) {
    if ptr != 0 {
        unsafe { drop(Vec::from_raw_parts(ptr as *mut u8, 0, len.max(0) as usize)) };
    }
}

#[no_mangle]
pub extern "C" fn pollek_plugin_reset() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn pollek_plugin_decide(ptr: i32, len: i32) -> i64 {
    let input = unsafe { core::slice::from_raw_parts(ptr as *const u8, len.max(0) as usize) };
    let out = run(input);
    let out_len = out.len() as i32;
    let out_ptr = alloc(out_len);
    unsafe { core::ptr::copy_nonoverlapping(out.as_ptr(), out_ptr as *mut u8, out.len()) };
    ((out_ptr as i64) << 32) | (out_len as i64 & 0xffff_ffff)
}

fn run(input: &[u8]) -> Vec<u8> {
    let req: Value = match serde_json::from_slice(input) {
        Ok(v) => v,
        Err(e) => return err(&format!("invalid input json: {e}")),
    };
    let to_contract = req
        .get("to_contract")
        .and_then(Value::as_str)
        .unwrap_or("2026.06.29")
        .to_string();
    let mut bundle = match req.get("bundle") {
        Some(b) => b.clone(),
        None => return err("missing 'bundle'"),
    };
    if !bundle.is_object() {
        return err("'bundle' must be an object");
    }

    let from_contract = bundle
        .get("compatibility")
        .and_then(|c| c.get("contract_version"))
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();

    let mut changes = Vec::<String>::new();
    migrate(&mut bundle, &to_contract, &mut changes);

    let out = json!({
        "adapted": !changes.is_empty(),
        "from_contract": from_contract,
        "to_contract": to_contract,
        "changes": changes,
        "bundle": bundle,
    });
    serde_json::to_vec(&out).unwrap_or_else(|_| b"{}".to_vec())
}

/// Deterministic, additive migration into the current contract shape. It never
/// drops author intent — it only fills required fields the target contract
/// expects and normalises shapes, recording each change.
fn migrate(bundle: &mut Value, to_contract: &str, changes: &mut Vec<String>) {
    let obj = match bundle.as_object_mut() {
        Some(o) => o,
        None => return,
    };

    if !obj.contains_key("apiVersion") {
        obj.insert("apiVersion".into(), json!("v1"));
        changes.push("set apiVersion=v1".into());
    }
    if !obj.contains_key("kind") {
        obj.insert("kind".into(), json!("PollekPolicyBundle"));
        changes.push("set kind=PollekPolicyBundle".into());
    }

    // compatibility block: ensure required fields the current contract expects.
    let compat = obj.entry("compatibility").or_insert_with(|| json!({}));
    if let Some(c) = compat.as_object_mut() {
        if !c.contains_key("min_dek_version") {
            c.insert("min_dek_version".into(), json!("1.0.0-beta.6"));
            changes.push("filled compatibility.min_dek_version".into());
        }
        for key in ["required_crates", "required_pep_types"] {
            if !c.contains_key(key) {
                c.insert(key.into(), json!([]));
                changes.push(format!("filled compatibility.{key}=[]"));
            }
        }
        let osm = c.entry("required_os_modules").or_insert_with(|| json!({}));
        if let Some(o) = osm.as_object_mut() {
            for plat in ["linux", "windows", "macos"] {
                if !o.contains_key(plat) {
                    o.insert(plat.into(), json!([]));
                    changes.push(format!("filled required_os_modules.{plat}=[]"));
                }
            }
        }
        // stamp the contract generation this bundle is now shaped for.
        let stamped = c
            .get("contract_version")
            .and_then(Value::as_str)
            .map(|v| v == to_contract)
            .unwrap_or(false);
        if !stamped {
            c.insert("contract_version".into(), json!(to_contract));
            changes.push(format!("stamped compatibility.contract_version={to_contract}"));
        }
    }

    // activation block defaults.
    let activation = obj.entry("activation").or_insert_with(|| json!({}));
    if let Some(a) = activation.as_object_mut() {
        let defaults = [
            ("strategy", json!("shadow")),
            ("rollback_on_failure", json!(true)),
            ("health_check_timeout_ms", json!(5000)),
            ("shadow_before_enforce_seconds", json!(30)),
        ];
        for (k, v) in defaults {
            if !a.contains_key(k) {
                a.insert(k.into(), v);
                changes.push(format!("filled activation.{k}"));
            }
        }
    }

    if !obj.contains_key("artifacts") {
        obj.insert("artifacts".into(), json!([]));
        changes.push("filled artifacts=[]".into());
    }
}

fn err(msg: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({ "adapted": false, "error": msg })).unwrap_or_else(|_| b"{}".to_vec())
}
