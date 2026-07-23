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
    /// auth-disabled local dev Cloud). If empty but the `oidc_*` fields are
    /// set, [`SyncConfig::ensure_bearer_token`] fetches one via the
    /// client-credentials grant.
    pub api_key: String,
    /// Subject attributed to ingested inventory (e.g. `DOMAIN\\user`).
    pub user_subject: String,
    /// OIDC token endpoint (Keycloak: `{issuer}/protocol/openid-connect/token`).
    /// Derived from `POLLEK_OIDC_TOKEN_URL` or `POLLEK_OIDC_ISSUER`.
    pub oidc_token_url: Option<String>,
    pub oidc_client_id: Option<String>,
    pub oidc_client_secret: Option<String>,
    pub oidc_scope: Option<String>,
    /// JWT-SVID presented as an OAuth `private_key_jwt` client assertion
    /// (RFC 7523). When set it is preferred over `client_secret`: the token
    /// exchange proves *which workload* is asking via its SPIFFE identity rather
    /// than a shared secret. Populated at runtime from the SPIRE JWT-SVID
    /// (`dek-spire-node::jwt_svid`) or `POLLEK_OIDC_CLIENT_ASSERTION`.
    pub oidc_client_assertion: Option<String>,
    /// The DEK's verified workload SPIFFE ID, read from the URI SAN of the
    /// provisioned X.509-SVID (`identity/svid.pem`) or `POLLEK_SPIFFE_ID`.
    /// Presented to Cloud on every request via the `x-pollek-spiffe-id` header
    /// (per the Cloud hand-off ask #3) so ingress can enforce
    /// `tenant/<id> == request tenant`. `None` in bearer/dev mode ⇒ header omitted.
    pub spiffe_id: Option<String>,
}

/// SPIFFE trust scheme the DEK and Cloud agreed on:
/// `spiffe://pollek.io/tenant/<tenant_id>/device/<device_id>` (agents:
/// `.../agent/<agent_id>`). Extract the authoritative tenant segment — the
/// binding Cloud enforces at ingress against the request tenant.
pub fn tenant_from_spiffe_id(spiffe_id: &str) -> Option<String> {
    let rest = spiffe_id.strip_prefix("spiffe://")?;
    let mut parts = rest.split('/');
    let _trust_domain = parts.next()?; // e.g. pollek.io
    while let Some(key) = parts.next() {
        if key == "tenant" {
            return parts.next().filter(|s| !s.is_empty()).map(str::to_string);
        }
    }
    None
}

/// The identity directory where the DEK keeps its SVID triple.
fn default_identity_dir() -> std::path::PathBuf {
    let base = std::env::var("DEK_LCP_DATA").unwrap_or_else(|_| "./pollek-local-data".into());
    std::path::PathBuf::from(base).join("identity")
}

/// Resolve the workload SPIFFE ID to present to Cloud: prefer the URI SAN of the
/// provisioned X.509-SVID (`identity_dir/svid.pem`), else `POLLEK_SPIFFE_ID`.
/// Never fabricated — returns `None` when neither exists (bearer/dev mode).
pub fn resolve_spiffe_id(identity_dir: &std::path::Path, now_unix: i64) -> Option<String> {
    if let Ok(pem) = std::fs::read_to_string(identity_dir.join("svid.pem")) {
        if let Ok(info) = dek_spire_node::describe_svid(&pem, now_unix) {
            if let Some(id) = info.spiffe_id.filter(|s| !s.is_empty()) {
                return Some(id);
            }
        }
    }
    std::env::var("POLLEK_SPIFFE_ID")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Keycloak/OIDC token endpoint path appended to a realm issuer URL.
const OIDC_TOKEN_PATH: &str = "/protocol/openid-connect/token";

/// Resolve the OIDC token endpoint from an explicit token URL or a realm
/// issuer (e.g. `https://keycloak/realms/pollek` → `.../protocol/openid-connect/token`).
fn resolve_oidc_token_url(token_url: Option<String>, issuer: Option<String>) -> Option<String> {
    if let Some(u) = token_url.filter(|v| !v.is_empty()) {
        return Some(u);
    }
    issuer
        .filter(|v| !v.is_empty())
        .map(|iss| format!("{}{}", iss.trim_end_matches('/'), OIDC_TOKEN_PATH))
}

/// RFC 7523 client-assertion type for `private_key_jwt` (JWT-SVID).
const JWT_BEARER_ASSERTION_TYPE: &str = "urn:ietf:params:oauth:client-assertion-type:jwt-bearer";

#[derive(serde::Deserialize)]
struct TokenResponse {
    access_token: String,
}

/// Build the `private_key_jwt` token-request form. Pure + testable: the assertion
/// (a JWT-SVID) proves the workload's SPIFFE identity instead of a shared secret.
fn assertion_form<'a>(
    client_id: &'a str,
    assertion: &'a str,
    scope: Option<&'a str>,
) -> Vec<(&'a str, &'a str)> {
    let mut form = vec![
        ("grant_type", "client_credentials"),
        ("client_id", client_id),
        ("client_assertion_type", JWT_BEARER_ASSERTION_TYPE),
        ("client_assertion", assertion),
    ];
    if let Some(scope) = scope {
        form.push(("scope", scope));
    }
    form
}

/// Exchange a JWT-SVID (OAuth `private_key_jwt` client assertion) for a bearer
/// token at the OIDC token endpoint (Keycloak). No shared secret leaves the DEK.
pub async fn client_assertion_token(
    client: &Client,
    token_url: &str,
    client_id: &str,
    assertion: &str,
    scope: Option<&str>,
) -> Result<String> {
    let form = assertion_form(client_id, assertion, scope);
    let resp = client
        .post(token_url)
        .form(&form)
        .send()
        .await
        .with_context(|| format!("POST {token_url} (private_key_jwt)"))?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        anyhow::bail!("OIDC token endpoint returned {status}: {text}");
    }
    let parsed: TokenResponse =
        serde_json::from_str(&text).with_context(|| format!("parse token response: {text}"))?;
    Ok(parsed.access_token)
}

/// Build the DEK↔Cloud transport client. When the device has a provisioned SVID
/// (the full triple under `identity_dir`), the transport is **mutual TLS** —
/// presenting the X.509-SVID as the client certificate. Otherwise a plain client
/// (bearer/dev). Never fabricates an identity; the presence of the SVID triple is
/// the single, honest signal.
pub fn build_transport(identity_dir: &std::path::Path) -> Result<Client> {
    if dek_spire_node::identity_present(identity_dir) {
        dek_spire_node::client_from_identity_dir(identity_dir)
    } else {
        Client::builder()
            .build()
            .map_err(|e| anyhow::anyhow!("build plain transport: {e}"))
    }
}

/// Fetch a bearer token via the OAuth2 client-credentials grant (Keycloak).
pub async fn client_credentials_token(
    client: &Client,
    token_url: &str,
    client_id: &str,
    client_secret: &str,
    scope: Option<&str>,
) -> Result<String> {
    let mut form = vec![
        ("grant_type", "client_credentials"),
        ("client_id", client_id),
        ("client_secret", client_secret),
    ];
    if let Some(scope) = scope {
        form.push(("scope", scope));
    }
    let resp = client
        .post(token_url)
        .form(&form)
        .send()
        .await
        .with_context(|| format!("POST {token_url} (client_credentials)"))?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        anyhow::bail!("OIDC token endpoint returned {status}: {text}");
    }
    let parsed: TokenResponse =
        serde_json::from_str(&text).with_context(|| format!("parse token response: {text}"))?;
    Ok(parsed.access_token)
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
            oidc_token_url: resolve_oidc_token_url(
                get("POLLEK_OIDC_TOKEN_URL").filter(|v| !v.is_empty()),
                get("POLLEK_OIDC_ISSUER").filter(|v| !v.is_empty()),
            ),
            oidc_client_id: get("POLLEK_OIDC_CLIENT_ID").filter(|v| !v.is_empty()),
            oidc_client_secret: get("POLLEK_OIDC_CLIENT_SECRET").filter(|v| !v.is_empty()),
            oidc_scope: get("POLLEK_OIDC_SCOPE").filter(|v| !v.is_empty()),
            oidc_client_assertion: get("POLLEK_OIDC_CLIENT_ASSERTION").filter(|v| !v.is_empty()),
            spiffe_id: resolve_spiffe_id(&default_identity_dir(), now_unix()),
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
            oidc_token_url: resolve_oidc_token_url(
                get("POLLEK_OIDC_TOKEN_URL"),
                get("POLLEK_OIDC_ISSUER"),
            ),
            oidc_client_id: get("POLLEK_OIDC_CLIENT_ID"),
            oidc_client_secret: get("POLLEK_OIDC_CLIENT_SECRET"),
            oidc_scope: get("POLLEK_OIDC_SCOPE"),
            oidc_client_assertion: get("POLLEK_OIDC_CLIENT_ASSERTION"),
            spiffe_id: resolve_spiffe_id(&default_identity_dir(), now_unix()),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.cloud_url, path)
    }

    /// Fail-closed tenant binding (Cloud hand-off ask #2). Cloud enforces that
    /// the caller's proven tenant equals the request tenant. When an SVID is
    /// present, its SPIFFE `tenant/<id>` segment is the proof and MUST equal
    /// `tenant_id`; a mismatch means the DEK would be asserting a tenant it
    /// cannot prove, so it refuses to sync rather than send a claim Cloud will
    /// (rightly) reject. With no SVID (bearer/dev) there is nothing to contradict.
    pub fn assert_tenant_binding(&self) -> Result<()> {
        if let Some(spiffe) = self.spiffe_id.as_deref() {
            if let Some(spiffe_tenant) = tenant_from_spiffe_id(spiffe) {
                if spiffe_tenant != self.tenant_id {
                    anyhow::bail!(
                        "tenant binding mismatch: SPIFFE tenant '{spiffe_tenant}' (from {spiffe}) \
                         != request tenant '{}'; refusing to present an unprovable tenant",
                        self.tenant_id
                    );
                }
            }
        }
        Ok(())
    }

    /// Ensure `api_key` holds a bearer token. If one was supplied explicitly
    /// (`DEK_CLOUD_API_KEY`), it is kept. Otherwise, when OIDC client
    /// credentials are configured, fetch a token via client-credentials and
    /// store it. Returns `Ok(true)` if a bearer is now set, `Ok(false)` if no
    /// auth is configured (valid only against an auth-disabled dev Cloud).
    pub async fn ensure_bearer_token(&mut self, client: &Client) -> Result<bool> {
        if !self.api_key.is_empty() {
            return Ok(true);
        }
        // Prefer private_key_jwt (JWT-SVID) — proves workload identity, no shared
        // secret — when both a token endpoint, client id, and assertion are set.
        if let (Some(url), Some(id), Some(assertion)) = (
            self.oidc_token_url.clone(),
            self.oidc_client_id.clone(),
            self.oidc_client_assertion.clone(),
        ) {
            let token =
                client_assertion_token(client, &url, &id, &assertion, self.oidc_scope.as_deref())
                    .await?;
            self.api_key = token;
            return Ok(true);
        }
        // Fall back to the client-credentials (shared-secret) grant.
        match (
            self.oidc_token_url.clone(),
            self.oidc_client_id.clone(),
            self.oidc_client_secret.clone(),
        ) {
            (Some(url), Some(id), Some(secret)) => {
                let token = client_credentials_token(
                    client,
                    &url,
                    &id,
                    &secret,
                    self.oidc_scope.as_deref(),
                )
                .await?;
                self.api_key = token;
                Ok(true)
            }
            _ => Ok(false),
        }
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
    // Present the verified workload SPIFFE ID (Cloud hand-off ask #3). Cloud's
    // trusted ingress enforces `tenant/<id> == request tenant` from this header.
    if let Some(spiffe) = cfg.spiffe_id.as_deref().filter(|s| !s.is_empty()) {
        req = req.header("x-pollek-spiffe-id", spiffe);
    }
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

    // Fail closed before any request leaves the device: never present a tenant
    // the SVID does not prove (Cloud hand-off ask #2).
    cfg.assert_tenant_binding()?;

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

    /// Minimal config for header/binding tests (no network).
    fn cfg_with(tenant: &str, spiffe: Option<&str>) -> SyncConfig {
        SyncConfig {
            cloud_url: "https://cloud.example".into(),
            tenant_id: tenant.into(),
            device_id: "device_1".into(),
            lcp_id: "lcp_1".into(),
            hostname: "h".into(),
            os: "linux".into(),
            os_family: "linux".into(),
            os_version: "1".into(),
            arch: "x86_64".into(),
            api_key: String::new(),
            user_subject: "local".into(),
            oidc_token_url: None,
            oidc_client_id: None,
            oidc_client_secret: None,
            oidc_scope: None,
            oidc_client_assertion: None,
            spiffe_id: spiffe.map(str::to_string),
        }
    }

    #[test]
    fn tenant_parsed_from_spiffe_device_and_agent_forms() {
        assert_eq!(
            tenant_from_spiffe_id("spiffe://pollek.io/tenant/acme/device/dv-9").as_deref(),
            Some("acme")
        );
        assert_eq!(
            tenant_from_spiffe_id("spiffe://pollek.io/tenant/acme/agent/ag-1").as_deref(),
            Some("acme")
        );
        // Malformed / missing tenant segment ⇒ None (no fabricated tenant).
        assert_eq!(
            tenant_from_spiffe_id("spiffe://pollek.io/device/dv-9"),
            None
        );
        assert_eq!(tenant_from_spiffe_id("https://pollek.io/tenant/acme"), None);
    }

    #[test]
    fn tenant_binding_fails_closed_on_mismatch() {
        // Matching SPIFFE tenant ⇒ ok.
        assert!(
            cfg_with("acme", Some("spiffe://pollek.io/tenant/acme/device/d"))
                .assert_tenant_binding()
                .is_ok()
        );
        // No SVID ⇒ nothing to contradict ⇒ ok (bearer/dev).
        assert!(cfg_with("acme", None).assert_tenant_binding().is_ok());
        // SPIFFE tenant != request tenant ⇒ refuse (fail closed).
        let err = cfg_with("acme", Some("spiffe://pollek.io/tenant/evil/device/d"))
            .assert_tenant_binding()
            .unwrap_err()
            .to_string();
        assert!(err.contains("tenant binding mismatch"), "{err}");
    }

    #[test]
    fn spiffe_id_header_presented_only_when_provisioned() {
        let client = Client::new();
        // Present when set.
        let req = apply_headers(
            client.post("https://cloud.example/enroll"),
            &cfg_with("acme", Some("spiffe://pollek.io/tenant/acme/device/d")),
        )
        .build()
        .unwrap();
        assert_eq!(
            req.headers()
                .get("x-pollek-spiffe-id")
                .and_then(|v| v.to_str().ok()),
            Some("spiffe://pollek.io/tenant/acme/device/d")
        );
        assert_eq!(
            req.headers()
                .get("x-pollek-tenant-id")
                .and_then(|v| v.to_str().ok()),
            Some("acme")
        );
        // Omitted in bearer/dev mode.
        let req2 = apply_headers(
            client.post("https://cloud.example/enroll"),
            &cfg_with("acme", None),
        )
        .build()
        .unwrap();
        assert!(req2.headers().get("x-pollek-spiffe-id").is_none());
    }

    #[test]
    fn private_key_jwt_form_is_rfc7523_shaped() {
        let form = assertion_form("dek-lcp", "eyJ.JWT.SVID", Some("pollek"));
        assert!(form.contains(&("grant_type", "client_credentials")));
        assert!(form.contains(&("client_id", "dek-lcp")));
        assert!(form.contains(&(
            "client_assertion_type",
            "urn:ietf:params:oauth:client-assertion-type:jwt-bearer"
        )));
        assert!(form.contains(&("client_assertion", "eyJ.JWT.SVID")));
        assert!(form.contains(&("scope", "pollek")));
        // No shared secret is ever present in the private_key_jwt exchange.
        assert!(!form.iter().any(|(k, _)| *k == "client_secret"));
    }

    #[test]
    fn assertion_form_omits_scope_when_absent() {
        let form = assertion_form("dek-lcp", "jwt", None);
        assert!(!form.iter().any(|(k, _)| *k == "scope"));
    }

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

    #[test]
    fn oidc_token_url_derived_from_keycloak_issuer() {
        let url = resolve_oidc_token_url(
            None,
            Some("https://keycloak-production-a39c.up.railway.app/realms/pollek".into()),
        );
        assert_eq!(
            url.as_deref(),
            Some("https://keycloak-production-a39c.up.railway.app/realms/pollek/protocol/openid-connect/token")
        );
    }

    #[test]
    fn oidc_explicit_token_url_wins_over_issuer() {
        let url = resolve_oidc_token_url(
            Some("https://issuer/token".into()),
            Some("https://keycloak/realms/pollek".into()),
        );
        assert_eq!(url.as_deref(), Some("https://issuer/token"));
        // No OIDC config at all ⇒ None (auth-disabled dev is allowed).
        assert_eq!(resolve_oidc_token_url(None, None), None);
    }

    #[test]
    fn token_response_parses_access_token() {
        let parsed: Result<TokenResponse, _> =
            serde_json::from_str(r#"{"access_token":"eyJhbGc","expires_in":300}"#);
        assert_eq!(
            parsed.ok().map(|t| t.access_token).as_deref(),
            Some("eyJhbGc")
        );
    }
}
