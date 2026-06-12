#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::duplicated_attributes
)]
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect
//
//! local_e2e.rs — full dual-mode loop against the LOCAL control plane:
//!
//!   author (create policy draft)
//!     -> publish (local-cp compiles + signs a bundle with its local key)
//!       -> enforce (DEK, pointed at local-cp, syncs + verifies + enforces)
//!         -> decision-log (DEK telemetry lands back in local-cp; dashboard reads it)
//!
//! This proves the invariant that the SAME DEK speaks the SAME contract to a
//! Local control plane as to Pollen Cloud — only endpoint + trust store differ.
//!
//! Run: cargo test -p acceptance-tests --test local_e2e -- --ignored --nocapture

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::time::sleep;

const LCP: &str = "http://127.0.0.1:3000";
const PEP: &str = "http://127.0.0.1:43890";

fn workspace_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}
fn bin(name: &str) -> PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(name)
        .with_extension(std::env::consts::EXE_EXTENSION)
}
fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap()
}

struct Proc(Child);
impl Drop for Proc {
    fn drop(&mut self) {
        let _ = self.0.start_kill();
    }
}

async fn wait_http(url: &str, tries: u32) -> Result<()> {
    let c = client();
    for _ in 0..tries {
        if c.get(url).send().await.is_ok() {
            return Ok(());
        }
        sleep(Duration::from_millis(500)).await;
    }
    anyhow::bail!("timeout waiting for {url}")
}

pub async fn poll_until<F, Fut>(
    timeout: Duration,
    interval: Duration,
    mut f: F,
) -> anyhow::Result<()>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if f().await {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            anyhow::bail!("poll_until timed out after {:?}", timeout);
        }
        tokio::time::sleep(interval).await;
    }
}

/// Fetch the local control plane's bundle-signing public key (base64) so the DEK
/// can be configured to trust it (the local equivalent of pinning the Cloud key).
async fn fetch_local_trust_key(c: &reqwest::Client) -> Result<String> {
    let v: serde_json::Value = c
        .get(format!("{LCP}/v1/tenants/local/devices/_/trusted-keys"))
        .send()
        .await?
        .json()
        .await?;
    v["keys"][0]["public_b64"]
        .as_str()
        .map(String::from)
        .context("local-cp trusted-keys missing public_b64")
}

/// authorize() -> (http_status, allow). Fail-closed: any parse failure => deny.
async fn authorize(
    c: &reqwest::Client,
    body: &serde_json::Value,
) -> (u16, bool, serde_json::Value) {
    match c
        .post(format!("{PEP}/v1/decision/check"))
        .json(body)
        .send()
        .await
    {
        Ok(r) => {
            let st = r.status().as_u16();
            let body_json = r
                .json::<serde_json::Value>()
                .await
                .unwrap_or(serde_json::json!({}));
            let allow = body_json
                .get("allow")
                .and_then(|a| a.as_bool())
                .unwrap_or(false);
            (st, allow, body_json)
        }
        Err(_) => (0, false, serde_json::json!({})),
    }
}

#[tokio::test]
#[ignore = "full local dual-mode e2e: author -> publish -> enforce -> decision-log"]
async fn local_e2e_author_publish_enforce_log() -> Result<()> {
    // ---- build once ----
    assert!(
        Command::new("cargo")
            .args(["build", "--workspace"])
            .status()
            .await?
            .success(),
        "workspace build failed"
    );

    // ---- start local control plane (single-user, tenant=local) ----
    let lcp_data = std::env::temp_dir().join(format!("lcp-e2e-{}", std::process::id()));
    let _lcp = Proc(
        Command::new(bin("local-control-plane"))
            .current_dir(workspace_dir())
            .env("DEK_LCP_DATA", &lcp_data)
            .env("DEK_LCP_DB", "sqlite::memory:")
            .env("DEK_LCP_AUTH_DISABLE", "1")
            .env("RUST_LOG", "info")
            .env_remove("DEK_PINNED_KEY_OVERRIDE")
            .spawn()
            .context("spawn local-control-plane")?,
    );
    wait_http(&format!("{LCP}/v1/tenants/local/registry/agents"), 20).await?;

    let c = client();

    // ======================================================================
    // STEP 1 — AUTHOR: create a Cedar policy draft (allow-all for the test)
    // ======================================================================
    let policy_id = "pol-e2e-allow";
    let now = "2026-06-09T00:00:00Z";
    let draft = serde_json::json!({
        "meta": {
            "schema_version": "1.0", "tenant_id": "local", "workspace_id": "default",
            "environment_id": "local", "created_at": now, "updated_at": now,
            "created_by": "local-admin", "updated_by": "local-admin",
            "source": "manual", "status": "draft", "tags": []
        },
        "policy_id": policy_id, "name": "e2e allow", "policy_type": "cedar",
        "targets": { "agent_ids": [], "tool_ids": [], "resource_ids": [], "entity_ids": [], "route_ids": [] },
        "source": { "kind": "raw_text", "language": "cedar", "text": "permit(principal, action, resource);" },
        "compile_options": { "fail_on_warnings": true }
    });
    let r = c
        .post(format!("{LCP}/v1/tenants/local/policies"))
        .json(&draft)
        .send()
        .await?;
    anyhow::ensure!(
        r.status().as_u16() == 201,
        "author: expected 201, got {}",
        r.status()
    );

    // ======================================================================
    // STEP 2 — PUBLISH: local-cp compiles + signs a bundle with its local key
    // ======================================================================
    let r = c
        .post(format!(
            "{LCP}/v1/tenants/local/policies/{policy_id}/publish"
        ))
        .json(&draft)
        .send()
        .await?;
    anyhow::ensure!(
        r.status().is_success(),
        "publish: expected 2xx, got {}",
        r.status()
    );
    let pub_body: serde_json::Value = r.json().await?;
    anyhow::ensure!(
        pub_body["published"] == true,
        "publish: not marked published: {pub_body}"
    );
    println!("[local_e2e] published bundle: {}", pub_body["bundle_id"]);

    // verify the signed manifest is fetchable on the same contract path Cloud uses
    let manifest: serde_json::Value = c
        .get(format!("{LCP}/v1/tenants/local/devices/_/bundles/manifest"))
        .send()
        .await?
        .json()
        .await?;
    anyhow::ensure!(
        manifest["signatures"]
            .as_array()
            .map(|s| !s.is_empty())
            .unwrap_or(false),
        "published manifest must be signed"
    );

    // ======================================================================
    // STEP 3 — ENFORCE: point the DEK at local-cp, enroll, sync, enforce
    // ======================================================================
    let trust_key = fetch_local_trust_key(&c).await?;
    let cfg = std::env::temp_dir().join(format!("dek-cfg-e2e-{}", std::process::id()));
    let data = std::env::temp_dir().join(format!("dek-data-e2e-{}", std::process::id()));
    let logs = std::env::temp_dir().join(format!("dek-logs-e2e-{}", std::process::id()));

    std::fs::create_dir_all(&cfg)?;
    std::fs::create_dir_all(&data)?;
    std::fs::create_dir_all(&logs)?;

    // profile -> local (writes bootstrap.json: cloud_url=LCP, tenant_id=local, trust=local key)
    let st = Command::new(bin("dek-cli"))
        .args([
            "profile",
            "set",
            "local",
            "--url",
            LCP,
            "--trusted-key",
            &trust_key,
        ])
        .env("DEK_CONFIG_DIR", &cfg)
        .env("DEK_DATA_DIR", &data)
        .env_remove("DEK_PINNED_KEY_OVERRIDE")
        .status()
        .await?;
    anyhow::ensure!(st.success(), "profile set local failed");

    // LCP doesn't implement /enroll (no need), so we just provide mock certs
    // to satisfy dek-core's mtls requirements during boot.
    let certs = cfg.join("certs");
    std::fs::create_dir_all(&certs)?;
    std::fs::write(
        certs.join("root_ca.crt"),
        "-----BEGIN CERTIFICATE-----\n\
MIIB0DCCAXWgAwIBAgIUC9VdKdxOfBnsn97H+gkVn42BHM4wCgYIKoZIzj0EAwIw\n\
PDEdMBsGA1UEAwwUUG9sbGVuIENsb3VkIFJvb3QgQ0ExGzAZBgNVBAoMElBvbGxl\n\
biBERUsgUHJvamVjdDAgFw03NTAxMDEwMDAwMDBaGA80MDk2MDEwMTAwMDAwMFow\n\
PDEdMBsGA1UEAwwUUG9sbGVuIENsb3VkIFJvb3QgQ0ExGzAZBgNVBAoMElBvbGxl\n\
biBERUsgUHJvamVjdDBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABA+RCCD6kluA\n\
a84Q5jayOGkwsDbyhwfAQxR7Q+AR1MYNLj22G8DV0hjjX0yH8vQr6mfC88dnLJVZ\n\
2igEXWgOpfyjUzBRMB8GA1UdEQQYMBaCFFBvbGxlbiBDbG91ZCBSb290IENBMB0G\n\
A1UdDgQWBBQL1V0p3E58Geyf3sf6CRWfjYEczjAPBgNVHRMBAf8EBTADAQH/MAoG\n\
CCqGSM49BAMCA0kAMEYCIQC5famYrlcNXrTyLT10TBc6SsRQkTFt5nHNErx9dOFo\n\
6gIhAMOiPmTL0rkB4RFvaGVcyje7Z3BVWCFgZ7lwuuoFzw6P\n\
-----END CERTIFICATE-----\n",
    )?;

    let _core = Proc(
        Command::new(bin("dek-core"))
            .env("DEK_CONFIG_DIR", &cfg)
            .env("DEK_DATA_DIR", &data)
            .env("DEK_LOG_DIR", &logs)
            .env("DEK_BUNDLE_SYNC_INTERVAL", "2")
            .env_remove("DEK_PINNED_KEY_OVERRIDE")
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .context("spawn dek-core")?,
    );
    wait_http(&format!("{PEP}/healthz"), 30).await?;
    let req = serde_json::json!({
        "request_id": "req-e2e-1",
        "tenant_id": "local",
        "device_id": "device-001",
        "principal": { "id": "e2e-user", "roles": [] },
        "action": "tools/call",
        "resource": { "resource_type": "mcp_tool", "resource_id": "safe.echo" },
        "context": {
            "mcp": { "method": "tools/call", "params": { "name": "safe.echo" } },
            "risk_tier": "low"
        },
        "input_hash": "dummy_hash"
    });

    poll_until(
        Duration::from_secs(30),
        Duration::from_millis(300),
        || async {
            let (st, allow, _body) = authorize(&c, &req).await;
            st == 200 && allow
        },
    )
    .await?;

    let (status, allow, body) = authorize(&c, &req).await;
    anyhow::ensure!(
        status == 200 && allow,
        "enforce: expected allow per published policy (status={status}, allow={allow}, body={body})"
    );
    println!("[local_e2e] enforce OK: allow={allow}");

    // ======================================================================
    // STEP 4 — DECISION LOG: the DEK's decision telemetry lands in local-cp
    // ======================================================================
    poll_until(
        Duration::from_secs(15),
        Duration::from_millis(300),
        || async {
            let r = c
                .get(format!("{LCP}/v1/tenants/local/telemetry/decision-logs"))
                .send()
                .await;
            if let Ok(resp) = r {
                if let Ok(logs) = resp.json::<serde_json::Value>().await {
                    return logs["decisions"]
                        .as_array()
                        .map(|a| !a.is_empty())
                        .unwrap_or(false);
                }
            }
            false
        },
    )
    .await?;

    let logs: serde_json::Value = c
        .get(format!("{LCP}/v1/tenants/local/telemetry/decision-logs"))
        .send()
        .await?
        .json()
        .await?;
    let decisions = logs["decisions"].as_array().cloned().unwrap_or_default();
    anyhow::ensure!(
        !decisions.is_empty(),
        "decision-log: expected at least one decision recorded in local-cp"
    );
    let has_allow = decisions.iter().any(|d| {
        d.pointer("/payload/decision").and_then(|v| v.as_str()) == Some("allow")
            || d.pointer("/mcp/verdict").and_then(|v| v.as_str()) == Some("allow")
    });
    anyhow::ensure!(
        has_allow,
        "decision-log: expected an 'allow' decision; got {decisions:?}"
    );
    println!(
        "[local_e2e] decision-log OK: {} decision(s) recorded",
        decisions.len()
    );

    println!("[local_e2e] PASS — author -> publish -> enforce -> decision-log");
    let _ = std::fs::remove_dir_all(&lcp_data);
    Ok(())
}
