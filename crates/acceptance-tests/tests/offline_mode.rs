// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::time::sleep;

const LCP: &str = "http://127.0.0.1:3005"; // Use a different port to avoid conflict with local_e2e
const PEP: &str = "http://127.0.0.1:43895";

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
        .timeout(Duration::from_secs(3))
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
#[ignore = "offline mode e2e"]
async fn test_offline_mode_resilience() -> Result<()> {
    assert!(
        Command::new("cargo")
            .args([
                "build",
                "-p",
                "local-control-plane",
                "-p",
                "dek-cli",
                "-p",
                "dek-core"
            ])
            .status()
            .await?
            .success(),
        "workspace build failed"
    );

    let lcp_data = std::env::temp_dir().join(format!("lcp-off-{}", std::process::id()));

    // Spawn LCP
    let mut lcp_proc = Command::new(bin("local-control-plane"))
        .current_dir(workspace_dir())
        .env("DEK_LCP_DATA", &lcp_data)
        .env("DEK_LCP_DB", "sqlite::memory:")
        .env("DEK_LCP_AUTH_DISABLE", "1")
        .env("DEK_LCP_BIND", "127.0.0.1:3005") // Use port 3005
        .env("RUST_LOG", "info")
        .env_remove("DEK_PINNED_KEY_OVERRIDE")
        .spawn()
        .context("spawn local-control-plane")?;

    wait_http(&format!("{LCP}/v1/tenants/local/registry/agents"), 20).await?;

    let c = client();

    let policy_id = "pol-offline-allow";
    let now = "2026-06-09T00:00:00Z";
    let draft = serde_json::json!({
        "meta": {
            "schema_version": "1.0", "tenant_id": "local", "workspace_id": "default",
            "environment_id": "local", "created_at": now, "updated_at": now,
            "created_by": "local-admin", "updated_by": "local-admin",
            "source": "manual", "status": "draft", "tags": []
        },
        "policy_id": policy_id, "name": "offline allow", "policy_type": "cedar",
        "targets": { "agent_ids": [], "tool_ids": [], "resource_ids": [], "entity_ids": [], "route_ids": [] },
        "source": { "kind": "raw_text", "language": "cedar", "text": "permit(principal, action, resource);" },
        "compile_options": { "fail_on_warnings": true }
    });

    let r = c
        .post(format!("{LCP}/v1/tenants/local/policies"))
        .json(&draft)
        .send()
        .await?;
    anyhow::ensure!(r.status().as_u16() == 201, "author: expected 201");

    let r = c
        .post(format!(
            "{LCP}/v1/tenants/local/policies/{policy_id}/publish"
        ))
        .json(&draft)
        .send()
        .await?;
    anyhow::ensure!(r.status().is_success(), "publish: expected 2xx");

    let trust_key = fetch_local_trust_key(&c).await?;
    let cfg = std::env::temp_dir().join(format!("dek-cfg-off-{}", std::process::id()));
    let data = std::env::temp_dir().join(format!("dek-data-off-{}", std::process::id()));

    std::fs::create_dir_all(&cfg)?;
    std::fs::create_dir_all(&data)?;

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

    let certs = cfg.join("certs");
    std::fs::create_dir_all(&certs)?;
    let ws_certs = workspace_dir().join("certs");
    std::fs::copy(ws_certs.join("root_ca.crt"), certs.join("root_ca.crt"))?;

    let _core = Proc(
        Command::new(bin("dek-core"))
            .env("DEK_CONFIG_DIR", &cfg)
            .env("DEK_DATA_DIR", &data)
            .env("DEK_API_PORT", "43895") // Use port 43895
            .env("DEK_BUNDLE_SYNC_INTERVAL", "2")
            .env_remove("DEK_PINNED_KEY_OVERRIDE")
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .context("spawn dek-core")?,
    );

    wait_http(&format!("{PEP}/healthz"), 30).await?;

    // Wait for the bundle to sync and policy to be active
    poll_until(
        Duration::from_secs(30),
        Duration::from_millis(300),
        || async {
            let req = serde_json::json!({
                "request_id": "req-off-1",
                "tenant_id": "local",
                "device_id": "device-001",
                "principal": { "id": "offline-user", "roles": [] },
                "action": "tools/call",
                "resource": { "resource_type": "mcp_tool", "resource_id": "safe.echo" },
                "context": { "risk_tier": "low" },
                "input_hash": "dummy"
            });
            let (st, allow, body) = authorize(&c, &req).await;
            if st != 200 || !allow {
                println!(
                    "poll_until auth failed: st={}, allow={}, body={}",
                    st, allow, body
                );
            }
            st == 200 && allow
        },
    )
    .await?;
    println!("[offline_mode] DEK successfully synced bundle and enforced policy online.");

    // KILL THE LOCAL CONTROL PLANE
    lcp_proc.kill().await?;
    println!("[offline_mode] Local control plane killed (simulating outage).");
    sleep(Duration::from_secs(3)).await; // wait a few seconds to let DEK notice the outage

    // Assert DEK still works using cached bundle
    let req = serde_json::json!({
        "request_id": "req-off-2",
        "tenant_id": "local",
        "device_id": "device-001",
        "principal": { "id": "offline-user", "roles": [] },
        "action": "tools/call",
        "resource": { "resource_type": "mcp_tool", "resource_id": "safe.echo" },
        "context": { "risk_tier": "low" },
        "input_hash": "dummy"
    });
    let (status, allow, body) = authorize(&c, &req).await;
    anyhow::ensure!(
        status == 200 && allow,
        "enforce: expected allow in offline mode using LKG bundle (status={status}, allow={allow}, body={body})"
    );
    println!(
        "[offline_mode] DEK successfully enforced policy OFFLINE using Last Known Good bundle."
    );

    println!("[offline_mode] PASS");
    let _ = std::fs::remove_dir_all(&lcp_data);
    Ok(())
}
