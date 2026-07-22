//! `cloud_sync_once` — run one full LCP → Pollek Cloud sync cycle on demand and
//! print the real Cloud responses. This is the runnable form of the sync client
//! (the "Wallet") described in `docs/HANDOFF_LCP_SYNC.md`.
//!
//! It performs the ordered, gated flow: enroll → inventory → telemetry →
//! usage-ledger, then (with `--verify`) reads back `/api/fleet`,
//! `/api/telemetry/ingest-status`, and `/api/reports/cost-tokens/overview`.
//!
//! Configuration comes from the environment (same vars the LCP already uses):
//!   DEK_CLOUD_URL         required, e.g. http://127.0.0.1:8790 or the Railway URL
//!   DEK_CLOUD_API_KEY     optional OAuth/OIDC bearer (omit only for auth-disabled dev)
//!   POLLEK_TENANT_ID      default "local"
//!   POLLEK_DEVICE_ID      default "device_local"
//!   POLLEK_LCP_ID         default "lcp_local"
//!   POLLEK_OS_FAMILY / POLLEK_OS_VERSION / POLLEK_HOSTNAME / POLLEK_ARCH / POLLEK_USER_SUBJECT
//!
//! Data sources (any subset):
//!   --snapshot <file.json>    inventory snapshot (the `snapshot` object)
//!   --telemetry <file.json>   array of telemetry-envelope.v1 events
//!   --ledger <file.json>      a pollek.lcp.usage-ledger.v1 document
//!   --sample                  use a small built-in representative payload set
//!   --verify                  GET the read endpoints after syncing
//!   --replay                  send the telemetry batch twice (idempotency demo)
//!
//! Exit code is non-zero if any step returns a non-2xx status.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use anyhow::{Context, Result};
use local_control_plane::cloud_sync_client::{
    contains_secret, enroll, ingest_inventory, make_envelope, push_telemetry_batch,
    push_usage_ledger, SyncConfig,
};
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |flag: &str| args.iter().any(|a| a == flag);
    let value_of = |flag: &str| -> Option<String> {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1).cloned())
    };

    let cfg = SyncConfig::from_env().context(
        "missing config — set DEK_CLOUD_URL (and POLLEK_TENANT_ID/DEVICE_ID/LCP_ID as needed)",
    )?;
    let client = Client::builder().timeout(Duration::from_secs(15)).build()?;
    let sample = has("--sample");

    println!("== cloud_sync_once ==");
    println!(
        "cloud={} tenant={} device={} lcp={}\n",
        cfg.cloud_url, cfg.tenant_id, cfg.device_id, cfg.lcp_id
    );

    let mut had_error = false;

    // Step 1 — enroll (REQUIRED FIRST; gates usage ledgers).
    let (status, body) = enroll(&client, &cfg).await?;
    print_step("1. enroll  POST /enroll", status, &body);
    had_error |= !ok(status);

    // Step 2 — inventory.
    let snapshot = load(value_of("--snapshot"))?.or_else(|| sample.then(sample_snapshot));
    if let Some(snapshot) = snapshot {
        let (status, body) = ingest_inventory(&client, &cfg, snapshot).await?;
        print_step("2. inventory  POST /api/entities/ingest", status, &body);
        had_error |= !ok(status);
    }

    // Step 3 — telemetry batch (with redaction guard + optional idempotency replay).
    let events: Option<Vec<Value>> = match load(value_of("--telemetry"))? {
        Some(Value::Array(a)) => Some(a),
        Some(_) => anyhow::bail!("--telemetry file must be a JSON array of envelopes"),
        None => sample.then(|| sample_events(&cfg)),
    };
    if let Some(events) = events {
        let dropped = events.iter().filter(|e| contains_secret(e)).count();
        if dropped > 0 {
            println!(
                "   (redaction guard: dropping {dropped} event(s) carrying a secret before send)"
            );
        }
        let batch_id = format!("batch_{}", uuid::Uuid::new_v4());
        let (status, body, dropped) =
            push_telemetry_batch(&client, &cfg, &batch_id, events.clone()).await?;
        print_step("3. telemetry  POST /v1/telemetry/batches", status, &body);
        println!("   dropped_for_secret={dropped}");
        had_error |= !ok(status);

        if has("--replay") {
            let replay_id = format!("batch_{}", uuid::Uuid::new_v4());
            let (status, body, _) = push_telemetry_batch(&client, &cfg, &replay_id, events).await?;
            print_step(
                "3b. telemetry REPLAY (same event_ids → expect duplicates, no double-count)",
                status,
                &body,
            );
            had_error |= !ok(status);
        }
    }

    // Step 4 — usage ledger (requires enrollment).
    let ledger = load(value_of("--ledger"))?.or_else(|| sample.then(|| sample_ledger(&cfg)));
    if let Some(ledger) = ledger {
        let (status, body) = push_usage_ledger(&client, &cfg, ledger).await?;
        print_step(
            "4. usage-ledger  POST /v1/tenants/{tenant}/lcp/usage-ledgers",
            status,
            &body,
        );
        had_error |= !ok(status);
    }

    // Optional read-side verification.
    if has("--verify") {
        println!("\n== verify (read side) ==");
        for (label, path) in [
            ("/api/fleet", "/api/fleet"),
            (
                "/api/telemetry/ingest-status",
                "/api/telemetry/ingest-status",
            ),
            (
                "/api/reports/cost-tokens/overview",
                "/api/reports/cost-tokens/overview",
            ),
        ] {
            match client
                .get(format!("{}{}", cfg.cloud_url, path))
                .send()
                .await
            {
                Ok(resp) => {
                    let s = resp.status().as_u16();
                    let v = resp.json::<Value>().await.unwrap_or(Value::Null);
                    print_step(label, s, &v);
                }
                Err(e) => println!("GET {label} -> error: {e}"),
            }
        }
    }

    if had_error {
        std::process::exit(1);
    }
    Ok(())
}

fn ok(status: u16) -> bool {
    (200..300).contains(&status)
}

fn print_step(label: &str, status: u16, body: &Value) {
    let mark = if ok(status) { "OK" } else { "FAIL" };
    println!("[{mark} {status}] {label}");
    let pretty = serde_json::to_string_pretty(body).unwrap_or_else(|_| body.to_string());
    for line in pretty.lines().take(24) {
        println!("      {line}");
    }
    println!();
}

fn load(path: Option<String>) -> Result<Option<Value>> {
    match path {
        None => Ok(None),
        Some(p) => {
            let text = std::fs::read_to_string(&p).with_context(|| format!("read {p}"))?;
            Ok(Some(
                serde_json::from_str(&text).with_context(|| format!("parse {p}"))?,
            ))
        }
    }
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn sample_snapshot() -> Value {
    json!({
        "agents": [{
            "agent_id": "agent_claude_code", "name": "Claude Code",
            "trust_level": "trusted", "declared_tools": ["tool_bash"], "declared_resources": ["res_repo"]
        }],
        "tools": [{ "tool_id": "tool_bash", "name": "Bash", "agent_id": "agent_claude_code" }],
        "resources": [{ "resource_id": "res_repo", "name": "Local Repo", "sensitivity": "internal" }],
        "entities": [],
        "relationships": [{ "from": "agent_claude_code", "to": "tool_bash", "label": "uses_tool" }],
        "candidates": [],
        "agent_inventory": []
    })
}

fn sample_events(cfg: &SyncConfig) -> Vec<Value> {
    let ts = now();
    vec![
        make_envelope(
            "evt_usage_sample_001",
            "ai_usage_event",
            &cfg.tenant_id,
            &cfg.device_id,
            &ts,
            json!({
                "agent_id": "agent_claude_code", "agent_name": "Claude Code",
                "user_subject": cfg.user_subject, "device_id": cfg.device_id, "lcp_id": cfg.lcp_id,
                "os_family": cfg.os_family, "os_version": cfg.os_version,
                "provider": "Anthropic", "model": "claude-sonnet-4",
                "tokens": { "input_tokens": 500, "output_tokens": 200, "total_tokens": 700, "cached_input_tokens": 0, "estimated": false },
                "cost": { "currency": "USD", "total_cost": 0.42 }
            }),
            true,
        ),
        make_envelope(
            "evt_decision_sample_001",
            "decision_log",
            &cfg.tenant_id,
            &cfg.device_id,
            &ts,
            json!({ "agent_id": "agent_claude_code", "decision": "allow", "resource": "res_repo", "action": "read", "matched_policy": "pol_default" }),
            true,
        ),
    ]
}

fn sample_ledger(cfg: &SyncConfig) -> Value {
    json!({
        "schema_version": "pollek.lcp.usage-ledger.v1",
        "ledger_id": format!("ledger_{}", uuid::Uuid::new_v4()),
        "tenant_id": cfg.tenant_id,
        "lcp_id": cfg.lcp_id,
        "device_id": cfg.device_id,
        "os_family": cfg.os_family,
        "os_version": cfg.os_version,
        "capture_method": "lcp_reported",
        "observed_at": now(),
        "usage_entries": [{
            "id": "uentry_sample_001",
            "agent_id": "agent_claude_code", "agent_name": "Claude Code",
            "device_id": cfg.device_id, "user_subject": cfg.user_subject,
            "provider": "Anthropic", "model": "claude-sonnet-4",
            "pricing_model": "token_metered", "allocation_method": "direct_token_meter",
            "call_count": 3, "input_tokens": 1500, "output_tokens": 600, "total_tokens": 2100,
            "allocated_cost_cents": 126, "currency": "USD", "confidence": "reported_by_lcp"
        }]
    })
}
