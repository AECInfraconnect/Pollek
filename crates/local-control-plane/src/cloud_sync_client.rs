//! LCP → Pollek Cloud sync client (the "Wallet" side).
//!
//! Implements the ordered, gated sync contract documented in the Pollek Cloud
//! hand-off (`docs/HANDOFF_LCP_SYNC.md`, contract `2026.07.13`):
//!
//!   1. `POST /enroll`                              — REQUIRED FIRST. An unknown
//!      LCP is rejected; usage ledgers from an unenrolled LCP get `400
//!      unknown_lcp:<id>`.
//!   2. `POST /api/entities/ingest`                 — inventory snapshot.
//!   3. `POST /v1/telemetry/batches`                — `telemetry-batch.v1` of
//!      `telemetry-envelope.v1` events (agent_observation, ai_usage_event,
//!      decision_log, tool_invocation, resource_access, enforcement_result,
//!      guard_incident, security_event).
//!   4. `POST /v1/tenants/{tenant}/lcp/usage-ledgers` — `pollek.lcp.usage-ledger.v1`.
//!
//! Gates honored here (never worked around):
//!   * Enroll before usage ledgers.
//!   * Idempotency: telemetry is keyed by `tenant_id + event_id`; the same
//!     `event_id` is reused on retry so replays are returned under `duplicates`
//!     and never double-count. Any 2xx is safe to clear the spool.
//!   * Redaction: `redaction_applied` is set honestly and events carrying a
//!     secret (`authorization:`, `bearer `, `"password"`) are dropped before
//!     send (the Cloud would otherwise quarantine them).
//!   * Consistent `tenant_id` / `device_id` / `lcp_id` on every call.
//!
//! The client is transport-only and decoupled from `AppState`, so it is unit
//! testable and usable both from the background loop and the `cloud_sync_once`
//! binary. In production the same calls carry an OAuth/OIDC bearer token (and,
//! at the transport layer, SPIFFE/mTLS); local dev may run auth-disabled.

use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::{json, Value};

/// Identity + endpoint configuration shared by every request.
#[derive(Debug, Clone)]
pub struct SyncConfig {
    pub cloud_url: String,
    pub tenant_id: String,
    pub device_id: String,
    pub lcp_id: String,
    pub hostname: String,
    pub os: String,
    pub os_family: String,
    pub os_version: String,
    pub arch: String,
    /// OAuth/OIDC bearer token. Empty ⇒ omit (only valid against an
    /// auth-disabled local dev Cloud).
    pub api_key: String,
    /// Subject attributed to ingested inventory (e.g. `DOMAIN\\user`).
    pub user_subject: String,
}

impl SyncConfig {
    /// Build from environment, matching the LCP's existing `DEK_CLOUD_*` /
    /// `POLLEK_*` variables. `cloud_url` (via `DEK_CLOUD_URL`) is required.
    pub fn from_env() -> Result<Self> {
        let get = |k: &str| std::env::var(k).ok().map(|v| v.trim().to_string());
        let cloud_url = get("DEK_CLOUD_URL")
            .filter(|v| !v.is_empty())
            .context("DEK_CLOUD_URL is required")?;
        let os_family = get("POLLEK_OS_FAMILY")
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| std::env::consts::OS.to_string());
        Ok(Self {
            cloud_url: cloud_url.trim_end_matches('/').to_string(),
            tenant_id: get("POLLEK_TENANT_ID")
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| "local".into()),
            device_id: get("POLLEK_DEVICE_ID")
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| "device_local".into()),
            lcp_id: get("POLLEK_LCP_ID")
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| "lcp_local".into()),
            hostname: get("POLLEK_HOSTNAME")
                .or_else(|| get("HOSTNAME"))
                .or_else(|| get("COMPUTERNAME"))
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| "pollek-lcp".into()),
            os: get("POLLEK_OS")
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| std::env::consts::OS.to_string()),
            os_family,
            os_version: get("POLLEK_OS_VERSION")
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| "unknown".into()),
            arch: get("POLLEK_ARCH")
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| std::env::consts::ARCH.to_string()),
            api_key: get("DEK_CLOUD_API_KEY").unwrap_or_default(),
            user_subject: get("POLLEK_USER_SUBJECT")
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| "local".into()),
        })
    }

    /// Build from an already-resolved cloud URL / identity (used by the
    /// background sync loop, which resolves `cloud_url`/`device_id` itself),
    /// filling host/OS/lcp fields from the environment with sensible defaults.
    pub fn for_context(
        cloud_url: String,
        tenant_id: String,
        device_id: String,
        api_key: String,
    ) -> Self {
        let get = |k: &str| {
            std::env::var(k)
                .ok()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        Self {
            cloud_url: cloud_url.trim_end_matches('/').to_string(),
            tenant_id,
            device_id,
            api_key,
            lcp_id: get("POLLEK_LCP_ID").unwrap_or_else(|| "lcp_local".into()),
            hostname: get("POLLEK_HOSTNAME")
                .or_else(|| get("HOSTNAME"))
                .or_else(|| get("COMPUTERNAME"))
                .unwrap_or_else(|| "pollek-lcp".into()),
            os: get("POLLEK_OS").unwrap_or_else(|| std::env::consts::OS.to_string()),
            os_family: get("POLLEK_OS_FAMILY").unwrap_or_else(|| std::env::consts::OS.to_string()),
            os_version: get("POLLEK_OS_VERSION").unwrap_or_else(|| "unknown".into()),
            arch: get("POLLEK_ARCH").unwrap_or_else(|| std::env::consts::ARCH.to_string()),
            user_subject: get("POLLEK_USER_SUBJECT").unwrap_or_else(|| "local".into()),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.cloud_url, path)
    }
}

/// Outcome of one full ordered sync cycle.
#[derive(Debug, Default)]
pub struct SyncReport {
    pub enrolled: bool,
    pub inventory_response: Option<Value>,
    pub telemetry_response: Option<Value>,
    pub ledger_response: Option<Value>,
    /// Events dropped locally because they carried a secret (redaction guard).
    pub redaction_dropped: usize,
}

/// Secret markers that must never leave the device unredacted. Kept in sync
/// with the Cloud's per-event quarantine rule.
const SECRET_MARKERS: [&str; 3] = ["authorization:", "bearer ", "\"password\""];

/// True if the serialized value contains a secret marker (case-insensitive).
/// Used both to set `redaction_applied` honestly and to drop events the Cloud
/// would quarantine.
pub fn contains_secret(payload: &Value) -> bool {
    let hay = payload.to_string().to_ascii_lowercase();
    SECRET_MARKERS.iter().any(|m| hay.contains(m))
}

/// Build a `telemetry-envelope.v1` with all required fields present.
/// `event_id` is the idempotency key — callers pass a **stable** id so retries
/// dedup rather than double-count. `redaction_applied` is computed honestly
/// from the payload contents unless the caller already redacted.
pub fn make_envelope(
    event_id: impl Into<String>,
    event_type: &str,
    tenant_id: &str,
    device_id: &str,
    timestamp: &str,
    payload: Value,
    already_redacted: bool,
) -> Value {
    let redaction_applied = already_redacted || contains_secret(&payload);
    json!({
        "schema_version": "telemetry-envelope.v1",
        "event_id": event_id.into(),
        "event_type": event_type,
        "timestamp": timestamp,
        "tenant_id": tenant_id,
        "device_id": device_id,
        "payload": payload,
        "redaction_applied": redaction_applied,
    })
}

fn apply_headers(mut req: reqwest::RequestBuilder, cfg: &SyncConfig) -> reqwest::RequestBuilder {
    req = req
        .header("content-type", "application/json")
        .header("x-pollek-tenant-id", &cfg.tenant_id)
        .header("x-pollek-device-id", &cfg.device_id)
        .header("x-pollek-lcp-id", &cfg.lcp_id);
    if !cfg.api_key.is_empty() {
        req = req.bearer_auth(&cfg.api_key);
    }
    req
}

async fn post_json(
    client: &Client,
    cfg: &SyncConfig,
    url: &str,
    body: &Value,
) -> Result<(u16, Value)> {
    let resp = apply_headers(client.post(url).json(body), cfg)
        .send()
        .await
        .with_context(|| format!("POST {url}"))?;
    let status = resp.status().as_u16();
    let text = resp.text().await.unwrap_or_default();
    let value = serde_json::from_str::<Value>(&text).unwrap_or_else(|_| json!({ "raw": text }));
    Ok((status, value))
}

/// Step 1 — enroll the LCP. REQUIRED before usage ledgers.
pub async fn enroll(client: &Client, cfg: &SyncConfig) -> Result<(u16, Value)> {
    let body = json!({
        "hostname": cfg.hostname,
        "device_id": cfg.device_id,
        "lcp_id": cfg.lcp_id,
        "os": cfg.os,
        "os_family": cfg.os_family,
        "os_version": cfg.os_version,
        "arch": cfg.arch,
        "capabilities": {},
    });
    post_json(client, cfg, &cfg.url("/enroll"), &body).await
}

/// Step 2 — push an inventory snapshot (agents/tools/resources/relationships).
/// `snapshot` is the `snapshot` object shape from the contract.
pub async fn ingest_inventory(
    client: &Client,
    cfg: &SyncConfig,
    snapshot: Value,
) -> Result<(u16, Value)> {
    let body = json!({
        "device_id": cfg.device_id,
        "lcp_id": cfg.lcp_id,
        "user_subject": cfg.user_subject,
        "snapshot": snapshot,
    });
    post_json(client, cfg, &cfg.url("/api/entities/ingest"), &body).await
}

/// Step 3 — push a telemetry batch. Events carrying a secret are dropped
/// locally (returned count) rather than sent for the Cloud to quarantine.
/// Returns `(status, response, dropped_for_secret)`.
pub async fn push_telemetry_batch(
    client: &Client,
    cfg: &SyncConfig,
    batch_id: &str,
    events: Vec<Value>,
) -> Result<(u16, Value, usize)> {
    let total = events.len();
    let safe: Vec<Value> = events.into_iter().filter(|e| !contains_secret(e)).collect();
    let dropped = total - safe.len();
    let body = json!({
        "schema_version": "telemetry-batch.v1",
        "tenant_id": cfg.tenant_id,
        "device_id": cfg.device_id,
        "batch_id": batch_id,
        "events": safe,
    });
    let (status, value) = post_json(client, cfg, &cfg.url("/v1/telemetry/batches"), &body).await?;
    Ok((status, value, dropped))
}

/// Step 4 — push a billing-grade usage ledger. Requires prior enrollment.
/// `ledger` is a full `pollek.lcp.usage-ledger.v1` document; identity fields
/// are overwritten from `cfg` so they always match the enrolled LCP.
pub async fn push_usage_ledger(
    client: &Client,
    cfg: &SyncConfig,
    mut ledger: Value,
) -> Result<(u16, Value)> {
    if let Some(obj) = ledger.as_object_mut() {
        obj.insert("schema_version".into(), json!("pollek.lcp.usage-ledger.v1"));
        obj.insert("tenant_id".into(), json!(cfg.tenant_id));
        obj.insert("lcp_id".into(), json!(cfg.lcp_id));
        obj.insert("device_id".into(), json!(cfg.device_id));
    }
    let url = cfg.url(&format!("/v1/tenants/{}/lcp/usage-ledgers", cfg.tenant_id));
    post_json(client, cfg, &url, &ledger).await
}

/// Run the full ordered flow: enroll → inventory → telemetry → usage ledger.
/// Skips any step whose input is `None`. Enrollment is always attempted first
/// so the usage-ledger gate is satisfied.
pub async fn run_full_sync_once(
    client: &Client,
    cfg: &SyncConfig,
    snapshot: Option<Value>,
    events: Option<Vec<Value>>,
    ledger: Option<Value>,
) -> Result<SyncReport> {
    let mut report = SyncReport::default();

    let (enroll_status, _enroll_body) = enroll(client, cfg).await?;
    report.enrolled = (200..300).contains(&enroll_status);

    if let Some(snapshot) = snapshot {
        let (_s, body) = ingest_inventory(client, cfg, snapshot).await?;
        report.inventory_response = Some(body);
    }
    if let Some(events) = events {
        let batch_id = format!("batch_{}", uuid::Uuid::new_v4());
        let (_s, body, dropped) = push_telemetry_batch(client, cfg, &batch_id, events).await?;
        report.telemetry_response = Some(body);
        report.redaction_dropped = dropped;
    }
    if let Some(ledger) = ledger {
        let (_s, body) = push_usage_ledger(client, cfg, ledger).await?;
        report.ledger_response = Some(body);
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_secret_markers_case_insensitively() {
        assert!(contains_secret(
            &json!({ "h": "Authorization: Bearer sk-1" })
        ));
        assert!(contains_secret(&json!({ "args": "BEARER abc" })));
        assert!(contains_secret(&json!({ "password": "hunter2" })));
        assert!(!contains_secret(
            &json!({ "agent_id": "a1", "tokens": 700 })
        ));
    }

    #[test]
    fn envelope_has_all_required_fields_and_honest_redaction() {
        let env = make_envelope(
            "evt_1",
            "ai_usage_event",
            "local",
            "device_1",
            "2026-07-13T01:00:00Z",
            json!({ "agent_id": "a1" }),
            false,
        );
        for field in [
            "schema_version",
            "event_id",
            "event_type",
            "timestamp",
            "tenant_id",
            "device_id",
            "payload",
            "redaction_applied",
        ] {
            assert!(env.get(field).is_some(), "missing {field}");
        }
        assert_eq!(env["schema_version"], "telemetry-envelope.v1");
        assert!(env["payload"].is_object());
        // Clean payload ⇒ redaction_applied false (honest).
        assert_eq!(env["redaction_applied"], json!(false));
    }

    #[test]
    fn envelope_marks_redaction_when_payload_has_secret() {
        let env = make_envelope(
            "evt_2",
            "tool_invocation",
            "local",
            "device_1",
            "2026-07-13T01:00:00Z",
            json!({ "cmd": "curl -H 'authorization: Bearer x'" }),
            false,
        );
        assert_eq!(env["redaction_applied"], json!(true));
    }

    #[test]
    fn stable_event_id_is_the_idempotency_key() {
        // Two envelopes built for the same logical event reuse the id, so the
        // Cloud dedups them by (tenant_id, event_id).
        let a = make_envelope(
            "evt_stable",
            "decision_log",
            "local",
            "d",
            "t",
            json!({}),
            true,
        );
        let b = make_envelope(
            "evt_stable",
            "decision_log",
            "local",
            "d",
            "t",
            json!({}),
            true,
        );
        assert_eq!(a["event_id"], b["event_id"]);
    }
}
