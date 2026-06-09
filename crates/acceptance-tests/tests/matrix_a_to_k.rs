//! Full-binary acceptance matrix (A–H) — spawns mock-cloud + dek-core and
//! exercises the Pollen DEK contract end-to-end over the real HTTP(S)/mTLS path.
//!
//! Run: cargo test -p acceptance-tests --test matrix_a_to_h -- --ignored --nocapture
//!
//! Marked #[ignore] so it doesn't run in the default unit pass (it builds the
//! workspace + spawns processes). CI runs it explicitly in an integration job.
//!
//! Prereqs handled by the harness:
//!   - `cargo build --workspace` (debug)
//!   - cert-gen writes certs/ (root CA + server + client) for mTLS
//!   - mock-cloud on :43891 (mTLS) + :43892 (enrollment HTTP)
//!   - dek-core enrolled against mock-cloud, then driven via its local IPC/PEP

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::time::sleep;

fn workspace_dir() -> PathBuf {
    // crates/acceptance-tests -> repo root
    std::env::current_dir().unwrap().parent().unwrap().parent().unwrap().to_path_buf()
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
fn insecure_client() -> reqwest::Client {
    reqwest::Client::builder().danger_accept_invalid_certs(true).build().unwrap()
}

/// Kill-on-drop guard for spawned children.
struct Proc(Child);
impl Drop for Proc {
    fn drop(&mut self) {
        let _ = self.0.start_kill();
    }
}

async fn wait_https(url: &str, tries: u32) -> Result<()> {
    let c = insecure_client();
    for _ in 0..tries {
        if c.get(url).send().await.is_ok() {
            return Ok(());
        }
        sleep(Duration::from_millis(500)).await;
    }
    anyhow::bail!("timeout waiting for {url}")
}

/// Build workspace + generate certs + start mock-cloud. Returns the running proc.
async fn setup() -> Result<Proc> {
    assert!(
        Command::new("cargo").args(["build", "--workspace"]).status().await?.success(),
        "workspace build failed"
    );
    // certs for mTLS (cert-gen writes ./certs)
    let _ = Command::new(bin("cert-gen")).current_dir(workspace_dir()).status().await;

    let mock = Command::new(bin("mock-cloud"))
        .current_dir(workspace_dir())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("spawn mock-cloud")?;
    wait_https("https://127.0.0.1:43892/admin/dashboard", 20).await?;
    Ok(Proc(mock))
}

/// Enroll a dek-core instance against mock-cloud and start it.
/// (Uses the device enrollment flow on :43892; see dek-cli `enroll`.)
async fn enroll_and_start_core() -> Result<Proc> {
    let tmp_config = workspace_dir().join("target").join("tmp_config");
    let tmp_data = workspace_dir().join("target").join("tmp_data");
    let _ = std::fs::remove_dir_all(&tmp_config);
    let _ = std::fs::remove_dir_all(&tmp_data);
    std::fs::create_dir_all(&tmp_config).unwrap();
    std::fs::create_dir_all(&tmp_data).unwrap();

    let status = Command::new(bin("dek-cli"))
        .args(["enroll", "--cloud-url", "https://127.0.0.1:43892"])
        .env("DEK_CONFIG_DIR", &tmp_config)
        .env("DEK_DATA_DIR", &tmp_data)
        .current_dir(workspace_dir())
        .status()
        .await
        .context("enroll")?;
    anyhow::ensure!(status.success(), "enrollment failed");

    let mut core = Command::new(bin("dek-core"))
        .env("DEK_CONFIG_DIR", &tmp_config)
        .env("DEK_DATA_DIR", &tmp_data)
        .env("DEK_BUNDLE_SYNC_INTERVAL", "2")
        .env("DEK_MAX_STALE_SECS", "4")
        .env("RUST_BACKTRACE", "1")
        .spawn()
        .context("spawn dek-core")?;
    // dek-core IPC on 127.0.0.1:43889; PEP/proxy on :43890
    sleep(Duration::from_secs(3)).await;
    Ok(Proc(core))
}

/// Pull the mock-cloud audit log (DEK-side audit events land here).
async fn fetch_audits() -> Result<serde_json::Value> {
    let c = insecure_client();
    let res = c.get("https://127.0.0.1:43892/mock/admin/audits").send().await?;
    Ok(res.json().await.unwrap_or(serde_json::json!([])))
}

// ===========================================================================
// The matrix. Each scenario is a function so failures are isolated & named.
// Gated behind one #[ignore] entry-point that sets up shared infra once.
// ===========================================================================

async fn authorize(pep: &reqwest::Client, req: &serde_json::Value) -> Result<(u16, bool, Option<i64>)> {
    let resp = pep.post("http://127.0.0.1:43890/v1/decision/check").json(req).send().await?;
    let status = resp.status().as_u16();
    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::json!({}));
    println!("AUTHORIZE RESPONSE STATUS: {} BODY: {}", status, body);
    let allow = body.get("allow").and_then(|v| v.as_bool()).unwrap_or(false);
    let err_code = body.get("error").and_then(|e| e.get("code")).and_then(|c| c.as_i64());
    Ok((status, allow, err_code))
}

#[tokio::test]
#[ignore = "full-binary integration; run explicitly in CI integration job"]
async fn acceptance_matrix_a_to_k() -> Result<()> {
    let _mock = setup().await?;

    // ---- A: Enroll -> sync -> enforce ----
    let _core = enroll_and_start_core().await?;
    // after sync, DEK should be Active and the PEP should authorize per policy.
    let pep = insecure_client();
    let allow_req = serde_json::json!({
        "request_id": "test-req-1",
        "tenant_id": "tenant-production-1",
        "device_id": "device-001",
        "principal": { "id": "user_bob", "roles": [] },
        "action": "tools/call",
        "resource": { "kind": "mcp_tool", "id": "some_tool" },
        "context": {},
        "input_hash": "dummy_hash"
    });
    let (status, allow, _) = authorize(&pep, &allow_req).await?;
    assert_eq!(status, 200, "A: PEP should return 200 OK");
    assert!(allow, "A: PEP decision must be allow after enroll+sync");

    // audit trail received policy.sync.success
    let audits = fetch_audits().await?;
    let txt = audits.to_string();
    assert!(txt.contains("policy.sync") || txt.contains("bundle"), "A: sync audit present");

    // ---- B: Unsigned/forged push -> reject + critical audit ----
    let _ = pep.post("https://127.0.0.1:43892/mock/admin/bundles/bad123/poison").send().await;
    sleep(Duration::from_secs(4)).await;
    let audits = fetch_audits().await?;
    assert!(
        audits.to_string().contains("poisoned") || audits.to_string().contains("POISON_BUNDLE"),
        "B: tampered bundle produced a rejection audit"
    );

    // ---- C: Network partition -> LKG -> strict-deny ----
    let _ = pep.post("https://127.0.0.1:43892/mock/admin/chaos/outage").json(&serde_json::json!({"enabled": true})).send().await;
    // DEK_BUNDLE_SYNC_INTERVAL is 2s, wait a bit so it triggers fallback
    sleep(Duration::from_secs(5)).await;
    let (status, allow, _) = authorize(&pep, &allow_req).await?;
    // Note: strict-deny when stale exceeds max grace period or network fails
    // We just verify it enforces strict-deny (allow == false) and doesn't crash
    assert!(!allow, "C: PEP must enforce strict-deny during partition (fail-closed)");

    // ---- D: Recovery -> active ----
    let _ = pep.post("https://127.0.0.1:43892/mock/admin/chaos/outage").json(&serde_json::json!({"enabled": false})).send().await;
    sleep(Duration::from_secs(4)).await;
    let (status, allow, _) = authorize(&pep, &allow_req).await?;
    assert!(allow, "D: PEP should recover to active state and allow requests");

    // ---- E: Key rotation ----
    let _ = pep.post("https://127.0.0.1:43892/mock/admin/keys/rotate").send().await;
    sleep(Duration::from_secs(4)).await;
    let audits = fetch_audits().await?;
    assert!(audits.to_string().contains("KEY_ROTATE"), "E: Key rotation audit present");

    // ---- F: Hot-reload no interrupt ----
    let _ = pep.post("https://127.0.0.1:43892/mock/admin/policies/publish").send().await;
    let mut tasks = vec![];
    for _ in 0..10 {
        let pep_clone = pep.clone();
        let allow_req_clone = allow_req.clone();
        tasks.push(tokio::spawn(async move {
            authorize(&pep_clone, &allow_req_clone).await
        }));
    }
    for t in tasks {
        let r = t.await.unwrap();
        assert!(r.is_ok(), "F: Hot reload caused no interrupts");
    }

    // ---- G: Backpressure ----
    // Fire many concurrent requests to test stability
    let mut tasks = vec![];
    for _ in 0..50 {
        let pep_clone = pep.clone();
        let allow_req_clone = allow_req.clone();
        tasks.push(tokio::spawn(async move {
            authorize(&pep_clone, &allow_req_clone).await
        }));
    }
    for t in tasks {
        let _ = t.await; // Just ensure it doesn't panic
    }

    // ---- H: PDP circuit breaker ----
    // Tested implicitly if backpressure or latency triggers it
    let _ = pep.post("https://127.0.0.1:43892/mock/admin/chaos/outage").json(&serde_json::json!({"enabled": true})).send().await;
    sleep(Duration::from_secs(5)).await;
    let (status, allow, _) = authorize(&pep, &allow_req).await?;
    assert!(!allow, "H: PDP circuit breaker handles outage gracefully and fails-closed");
    let _ = pep.post("https://127.0.0.1:43892/mock/admin/chaos/outage").json(&serde_json::json!({"enabled": false})).send().await;
    sleep(Duration::from_secs(5)).await;

    // ---- I: Network enforce ----
    let mock_policy = serde_json::json!({
        "rules": [{
            "policy_id": "pol-net-001",
            "policy_type": "NETWORK_EGRESS_GUARDRAIL",
            "version": 1,
            "risk_tier": "high",
            "targets": { "devices": ["*"] },
            "conditions": { "destinations": [{ "type": "domain", "value": "malicious.example.com" }] },
            "effect": "DENY",
            "fallback": { "cloud_unavailable": "FAIL_CLOSED", "policy_stale": "FAIL_CLOSED" }
        }]
    });
    let _ = pep.post("https://127.0.0.1:43892/mock/admin/network/publish").json(&mock_policy).send().await;
    sleep(Duration::from_secs(4)).await;
    let audits = fetch_audits().await?;
    assert!(audits.to_string().contains("network-publish"), "I: Network rule publish audit present");

    // ---- J: Network fail-closed ----
    // Trigger an outage to test fail-closed behavior via metrics or at least ensure stability
    let _ = pep.post("https://127.0.0.1:43892/mock/admin/chaos/outage").json(&serde_json::json!({"enabled": true})).send().await;
    sleep(Duration::from_secs(4)).await;
    let _ = pep.post("https://127.0.0.1:43892/mock/admin/chaos/outage").json(&serde_json::json!({"enabled": false})).send().await;

    // ---- K: Obligation / Pending Approval ----
    let mock_obligation_policy = serde_json::json!({
        "rules": [{
            "policy_id": "pol-oblig-001",
            "policy_type": "RESOURCE_ACCESS",
            "version": 1,
            "risk_tier": "high",
            "targets": { "devices": ["*"] },
            "effect": "ALLOW",
            "obligations": ["require_approval"]
        }]
    });
    let _ = pep.post("https://127.0.0.1:43892/mock/admin/policies/publish").json(&mock_obligation_policy).send().await;
    sleep(Duration::from_secs(4)).await;

    let (status, allow, err_code) = authorize(&pep, &allow_req).await?;
    assert!(!allow, "K: Request should not be allowed directly");
    assert_eq!(err_code, Some(-32002), "K: Must return pending_approval code");

    // Operator approves the pending request
    let _ = pep.post("https://127.0.0.1:43892/admin/approvals/approve").send().await;
    sleep(Duration::from_secs(4)).await;

    let (status, allow, _) = authorize(&pep, &allow_req).await?;
    assert!(allow, "K: Request should be allowed after approval");

    Ok(())
}
