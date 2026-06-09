//! R4 Soak Test Harness
//! Runs a continuous load against mock-cloud and dek-core, 
//! injecting chaos and monitoring RSS memory growth to detect leaks.
//! Run with: cargo test --test soak -- --ignored --nocapture

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};
use sysinfo::{Pid, System};
use tokio::process::{Child, Command};
use tokio::time::sleep;

fn workspace_dir() -> PathBuf {
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
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .pool_max_idle_per_host(50)
        .build()
        .unwrap()
}

struct Proc(Child, Option<u32>);
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

async fn setup_mock_cloud() -> Result<Proc> {
    assert!(
        Command::new("cargo").args(["build", "--workspace"]).status().await?.success(),
        "workspace build failed"
    );
    let _ = Command::new(bin("cert-gen")).current_dir(workspace_dir()).status().await;

    let mut mock = Command::new(bin("mock-cloud"))
        .current_dir(workspace_dir())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("spawn mock-cloud")?;
    let pid = mock.id();
    wait_https("https://127.0.0.1:43892/admin/dashboard", 20).await?;
    Ok(Proc(mock, pid))
}

async fn enroll_and_start_core() -> Result<Proc> {
    let tmp_config = workspace_dir().join("target").join("tmp_config_soak");
    let tmp_data = workspace_dir().join("target").join("tmp_data_soak");
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
        .spawn()
        .context("spawn dek-core")?;
    let pid = core.id();
    sleep(Duration::from_secs(3)).await;
    Ok(Proc(core, pid))
}

async fn authorize(pep: &reqwest::Client, req: &serde_json::Value) -> Result<(u16, bool)> {
    let resp = pep.post("http://127.0.0.1:43890/v1/decision/check").json(req).send().await?;
    let status = resp.status().as_u16();
    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::json!({}));
    let allow = body.get("decision").and_then(|v| v.as_str()) == Some("allow");
    Ok((status, allow))
}

#[tokio::test]
#[ignore = "long running soak test"]
async fn soak_harness() -> Result<()> {
    // Determine soak duration
    let soak_secs: u64 = std::env::var("SOAK_SECS")
        .unwrap_or_else(|_| "60".to_string())
        .parse()
        .unwrap_or(60);

    println!("Starting soak test for {} seconds", soak_secs);
    let _mock = setup_mock_cloud().await?;
    let core = enroll_and_start_core().await?;
    
    let core_pid = core.1.expect("dek-core pid not found");
    let mut sys = System::new_all();
    sys.refresh_processes();
    let initial_rss = sys.process(Pid::from_u32(core_pid)).map(|p| p.memory()).unwrap_or(0);
    println!("Initial dek-core RSS: {} KB", initial_rss);

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

    let start = Instant::now();
    let mut total_reqs = 0;
    let mut err_reqs = 0;
    
    let mut last_chaos = Instant::now();
    let chaos_interval = Duration::from_secs(15);
    
    while start.elapsed() < Duration::from_secs(soak_secs) {
        total_reqs += 1;
        match authorize(&pep, &allow_req).await {
            Ok((status, _)) => {
                if status != 200 && status != 503 {
                    err_reqs += 1;
                }
            }
            Err(_) => {
                err_reqs += 1;
            }
        }
        
        // Inject chaos periodically
        if last_chaos.elapsed() > chaos_interval {
            println!("Injecting chaos (network outage)...");
            let _ = pep.post("https://127.0.0.1:43892/mock/admin/chaos/outage").json(&serde_json::json!({"enabled": true})).send().await;
            sleep(Duration::from_secs(3)).await;
            let _ = pep.post("https://127.0.0.1:43892/mock/admin/chaos/outage").json(&serde_json::json!({"enabled": false})).send().await;
            
            println!("Rotating keys...");
            let _ = pep.post("https://127.0.0.1:43892/mock/admin/keys/rotate").send().await;
            
            last_chaos = Instant::now();
        }
        
        sleep(Duration::from_millis(50)).await;
    }
    
    let err_rate = (err_reqs as f64) / (total_reqs as f64);
    println!("Soak completed. Total requests: {}, Errors: {}, Error rate: {:.2}%", total_reqs, err_reqs, err_rate * 100.0);
    assert!(err_rate < 0.10, "Error rate must be < 10%");
    
    sys.refresh_processes();
    let final_rss = sys.process(Pid::from_u32(core_pid)).map(|p| p.memory()).unwrap_or(0);
    println!("Final dek-core RSS: {} KB", final_rss);
    
    // Check for leak: RSS growth > 1.5x (and significantly > 10MB to avoid base noise)
    if initial_rss > 10_000 && final_rss > (initial_rss as f64 * 1.5) as u64 {
        panic!("Potential memory leak detected! RSS grew from {} KB to {} KB", initial_rss, final_rss);
    }
    
    Ok(())
}
