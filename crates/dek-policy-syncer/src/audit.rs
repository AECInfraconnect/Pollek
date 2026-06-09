//! audit.rs — tamper-evident audit trail for every policy lifecycle event (Phase 3).
//!
//! Every event carries a monotonic `seq` and `prev_digest` forming a hash chain
//! so a SIEM can detect gaps or back-dated edits. Events are emitted via the
//! telemetry sink (durable spool) with priority. Critical security events
//! (unsigned/forged bundle) use Priority::Critical so they survive spool eviction.

use dek_telemetry::{CloudTelemetrySink, Priority};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub struct AuditTrail {
    sink: Option<Arc<CloudTelemetrySink>>,
    seq: AtomicU64,
    prev_digest: Mutex<String>,
    device_id: String,
    tenant_id: String,
}

impl AuditTrail {
    pub fn new(sink: Option<Arc<CloudTelemetrySink>>, device_id: String, tenant_id: String) -> Self {
        Self {
            sink,
            seq: AtomicU64::new(0),
            prev_digest: Mutex::new("genesis".to_string()),
            device_id,
            tenant_id,
        }
    }

    fn emit(&self, event_type: &str, severity: &str, mut payload: Value, priority: Priority) {
        let seq = self.seq.fetch_add(1, Ordering::SeqCst);
        let prev = self.prev_digest.lock().map(|g| g.clone()).unwrap_or_default();
        let ts = chrono_now_rfc3339();

        if let Some(obj) = payload.as_object_mut() {
            obj.insert("event_type".into(), json!("audit"));
            obj.insert("action".into(), json!(event_type));
            obj.insert("severity".into(), json!(severity));
            obj.insert("timestamp".into(), json!(ts)); // use timestamp instead of ts for TelemetryEvent
            obj.insert("seq".into(), json!(seq));
            obj.insert("prev_digest".into(), json!(prev));
            obj.insert("device_id".into(), json!(self.device_id));
            obj.insert("tenant_id".into(), json!(self.tenant_id));
        }

        // chain: next prev_digest = sha256(prev || canonical(payload))
        let digest = {
            let mut h = Sha256::new();
            h.update(prev.as_bytes());
            h.update(serde_json::to_vec(&payload).unwrap_or_default());
            format!("sha256:{}", hex::encode(h.finalize()))
        };
        if let Ok(mut g) = self.prev_digest.lock() {
            *g = digest;
        }

        if let Some(sink) = &self.sink {
            sink.emit_async(payload, priority);
        }
    }

    // ---- event constructors ----

    pub fn sync_success(&self, version: &str, key_id: &str, signature_digest: &str) {
        self.emit(
            "policy.sync.success",
            "info",
            json!({ "bundle_version": version, "signing_key_id": key_id, "signature_digest": signature_digest, "result": "success" }),
            Priority::High,
        );
    }

    /// SECURITY: emitted when a bundle fails signature verification — i.e. a
    /// possible attempt to push an unsigned/forged bundle to this device.
    pub fn unsigned_bundle_rejected(&self, role: &str, reason: &str) {
        self.emit(
            "policy.sync.rejected",
            "critical",
            json!({ "role": role, "reason": reason, "result": "rejected" }),
            Priority::Critical,
        );
    }

    pub fn rollback_blocked(&self, current_gen: u64, incoming_gen: u64) {
        self.emit(
            "policy.sync.rejected",
            "critical",
            json!({ "reason": "anti_rollback", "current_generation": current_gen, "incoming_generation": incoming_gen, "result": "rejected" }),
            Priority::Critical,
        );
    }

    pub fn activated(&self, version: &str, mode: &str) {
        self.emit(
            "policy.activate",
            "info",
            json!({ "bundle_version": version, "mode": mode, "result": "success" }),
            Priority::High,
        );
    }

    pub fn state_change(&self, from: &str, to: &str, reason: &str) {
        let sev = if to == "strict_deny" { "critical" } else { "warning" };
        let pri = if to == "strict_deny" { Priority::Critical } else { Priority::High };
        self.emit(
            "policy.enforcement.state_change",
            sev,
            json!({ "from": from, "to": to, "reason": reason }),
            pri,
        );
    }

    pub fn key_rotation(&self, added: &[String], promoted: &[String], revoked: &[String]) {
        self.emit(
            "policy.key_rotation",
            "warning",
            json!({ "added_key_ids": added, "promoted_key_ids": promoted, "revoked_key_ids": revoked }),
            Priority::High,
        );
    }
}

fn chrono_now_rfc3339() -> String {
    // Keep dependency-light: seconds since epoch in RFC3339-ish if chrono absent.
    // If chrono is already a workspace dep, prefer chrono::Utc::now().to_rfc3339().
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!("unix:{secs}")
}
