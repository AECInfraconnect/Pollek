// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

#![warn(clippy::print_stdout, clippy::print_stderr)]
#![deny(clippy::unwrap_used, clippy::expect_used)]
#![forbid(unsafe_code)]
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod explanation;

#[derive(Debug, Error)]
pub enum PolicyError {
    /// PDP backend is unavailable (network/timeout)
    #[error("policy backend unavailable: {0}")]
    Unavailable(String),
    /// Invalid policy or input format (parse/validation)
    #[error("invalid policy or input: {0}")]
    Invalid(String),
    /// Other evaluation errors
    #[error("evaluation error: {0}")]
    Eval(String),
}

pub type PolicyResult = std::result::Result<PolicyDecision, PolicyError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyDecision {
    pub evaluator_id: String,
    pub evaluator_type: String,
    pub required: bool,
    pub status: String,
    pub decision: String,
    pub allow: bool,
    pub reason: String,
    pub effects: serde_json::Value,
    pub obligations: Vec<String>,
    pub metadata: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explanation: Option<explanation::DecisionExplanation>,
    #[serde(default)]
    pub user_action_required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_action_th: Option<String>,
}

use async_trait::async_trait;

#[async_trait]
pub trait PolicyRuntime: Send + Sync {
    async fn evaluate(&self, input: std::sync::Arc<serde_json::Value>) -> PolicyResult;
    fn version(&self) -> String;
    async fn clear_cache(&self) {}
}

// Replaced with uniform PolicyDecision schema

/// A Mock Runtime for Phase 2 that simulates an OPA policy evaluation
/// This is highly testable without needing a full WebAssembly ABI integration initially.
pub struct MockPolicyRuntime;

#[async_trait]
impl PolicyRuntime for MockPolicyRuntime {
    async fn evaluate(&self, input: std::sync::Arc<serde_json::Value>) -> PolicyResult {
        // A simple mock matching the SRS Appendix A policy:
        // allow if mcp.method == "tools/call" and mcp.tool_name == "safe.echo"
        let allow = if let Some(mcp) = input.get("mcp") {
            if let Some(tool) = mcp.get("tool_name") {
                tool.as_str() == Some("safe.echo")
            } else {
                false
            }
        } else {
            false
        };

        Ok(PolicyDecision {
            evaluator_id: "opa_wasm_mock".to_string(),
            evaluator_type: "local_pdp".to_string(),
            required: true,
            status: "success".to_string(),
            decision: if allow {
                "allow".to_string()
            } else {
                "deny".to_string()
            },
            allow,
            reason: if allow {
                "allowed by guardrail policy".to_string()
            } else {
                "tool is not allowed".to_string()
            },
            effects: serde_json::json!({ "audit": true }),
            obligations: vec!["write_decision_log".to_string()],
            metadata: serde_json::json!({ "version": "mock-v0.1.0" }),
            explanation: None,
            user_action_required: false,
            user_action_th: None,
        })
    }

    fn version(&self) -> String {
        "mock-v0.1.0".to_string()
    }
}

use anyhow::Result;
use wasmtime::*;
use wasmtime_wasi::p1;
use wasmtime_wasi::p2::pipe::{MemoryInputPipe, MemoryOutputPipe};
use wasmtime_wasi::WasiCtxBuilder;

/// WASM execution profile defining resource limits
#[derive(Debug, Clone)]
pub struct WasmProfile {
    pub max_memory_bytes: usize,
    pub max_fuel: u64,
}

impl Default for WasmProfile {
    fn default() -> Self {
        Self {
            max_memory_bytes: 10 * 1024 * 1024, // 10 MB default
            max_fuel: 1_000_000,                // 1M instructions default
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdpProbeResult {
    pub status: String,
    pub latency_ms: u64,
    pub message: String,
}

pub async fn probe_local_pdp(runtime: &dyn PolicyRuntime) -> PdpProbeResult {
    let input = serde_json::json!({
        "principal": "agent:test",
        "action": "tool.call",
        "resource": "tool:test",
        "context": { "probe": true }
    });

    let started = std::time::Instant::now();
    match runtime.evaluate(std::sync::Arc::new(input)).await {
        Ok(decision) => PdpProbeResult {
            status: "ready".into(),
            latency_ms: started.elapsed().as_millis() as u64,
            message: format!("probe decision: {}", decision.allow),
        },
        Err(err) => PdpProbeResult {
            status: "misconfigured".into(),
            latency_ms: started.elapsed().as_millis() as u64,
            message: err.to_string(),
        },
    }
}

struct RuntimeState {
    wasi: wasmtime_wasi::p1::WasiP1Ctx,
    limits: StoreLimits,
}

/// The actual WASM runtime host
pub struct WasmtimePolicyRuntime {
    engine: Engine,
    instance_pre: InstancePre<RuntimeState>,
    wasm_path: String,
    profile: WasmProfile,
}

impl WasmtimePolicyRuntime {
    pub fn new(wasm_path: &str, profile: Option<WasmProfile>) -> Result<Self> {
        let profile = profile.unwrap_or_default();

        let mut config = Config::new();
        config.consume_fuel(true);
        config.max_wasm_stack(1024 * 1024); // 1 MB stack limit

        let engine =
            Engine::new(&config).map_err(|e| ::anyhow::anyhow!("Failed to init Engine: {}", e))?;
        let module = Module::from_file(&engine, wasm_path)
            .map_err(|e| ::anyhow::anyhow!("Failed to load WASM module: {}", e))?;

        let mut linker = Linker::new(&engine);
        // Link WASI preview 1 functions to our custom state
        p1::add_to_linker_sync(&mut linker, |s: &mut RuntimeState| &mut s.wasi)
            .map_err(|e| ::anyhow::anyhow!("Failed to link WASI preview1: {e}"))?;
        let instance_pre = linker
            .instantiate_pre(&module)
            .map_err(|e| ::anyhow::anyhow!("Failed to pre-instantiate module: {e}"))?;

        Ok(Self {
            engine,
            instance_pre,
            wasm_path: wasm_path.to_string(),
            profile,
        })
    }
}

#[async_trait]
impl PolicyRuntime for WasmtimePolicyRuntime {
    async fn evaluate(&self, input: std::sync::Arc<serde_json::Value>) -> PolicyResult {
        let input_str =
            serde_json::to_string(&*input).map_err(|e| PolicyError::Invalid(e.to_string()))?;
        let stdin = MemoryInputPipe::new(bytes::Bytes::from(input_str.into_bytes()));
        let stdout = MemoryOutputPipe::new(self.profile.max_memory_bytes);

        let mut builder = WasiCtxBuilder::new();
        builder.stdin(stdin.clone());
        builder.stdout(stdout.clone());
        builder.inherit_stderr(); // For debugging
        let wasi = builder.build_p1();

        let limits = StoreLimitsBuilder::new()
            .memory_size(self.profile.max_memory_bytes)
            .build();

        let state = RuntimeState { wasi, limits };
        let mut store = Store::new(&self.engine, state);

        store.limiter(|state| &mut state.limits);

        store
            .set_fuel(self.profile.max_fuel)
            .map_err(|e| PolicyError::Eval(format!("failed to set fuel: {e}")))?;

        // Run plugin from pre-compiled module (thread-safe, concurrent)
        let instance = self
            .instance_pre
            .instantiate(&mut store)
            .map_err(|e| PolicyError::Eval(format!("instantiate: {e}")))?;
        let func = instance
            .get_typed_func::<(), ()>(&mut store, "_start")
            .map_err(|e| PolicyError::Eval(format!("get _start: {e}")))?;

        let mut reason = "Executed WASM policy".to_string();
        let mut allow = false;

        match func.call(&mut store, ()) {
            Ok(_) => {
                // Read result from stdout memory pipe
                let out_bytes = stdout.contents();

                let output_str = String::from_utf8_lossy(&out_bytes);

                if let Ok(output_val) = serde_json::from_str::<serde_json::Value>(&output_str) {
                    allow = output_val
                        .get("allow")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if let Some(r) = output_val.get("reason").and_then(|v| v.as_str()) {
                        reason = r.to_string();
                    }
                } else {
                    reason = format!("Failed to parse WASM output JSON: {}", output_str);
                }
            }
            Err(e) => {
                // Classify the wasm trap by type rather than Display text,
                // which is not stable across wasmtime versions. Out-of-fuel is
                // the CPU/instruction guardrail; surface it explicitly so audit
                // logs distinguish a resource-limit stop from other traps.
                reason = match e.downcast_ref::<wasmtime::Trap>() {
                    Some(wasmtime::Trap::OutOfFuel) => {
                        "WASM execution failed: out of fuel (CPU/instruction limit exceeded)"
                            .to_string()
                    }
                    Some(trap) => format!("WASM execution failed: wasm trap: {trap}"),
                    None => format!("WASM execution failed: {e:#}"),
                };
            }
        }

        Ok(PolicyDecision {
            evaluator_id: "opa_wasm_native".to_string(),
            evaluator_type: "wasm_pdp".to_string(),
            required: true,
            status: "success".to_string(),
            decision: if allow {
                "allow".to_string()
            } else {
                "deny".to_string()
            },
            allow,
            reason,
            effects: serde_json::json!({}),
            obligations: vec![],
            metadata: serde_json::json!({ "wasm_path": self.wasm_path }),
            explanation: None,
            user_action_required: false,
            user_action_th: None,
        })
    }

    fn version(&self) -> String {
        "wasm-native-v1.1.0".to_string()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_mock_policy_allow() {
        let runtime = MockPolicyRuntime;
        let input = json!({
            "mcp": {
                "method": "tools/call",
                "tool_name": "safe.echo"
            }
        });

        let decision = runtime.evaluate(std::sync::Arc::new(input)).await.unwrap(); //
        assert!(decision.allow);
        assert_eq!(decision.reason, "allowed by guardrail policy");
    }

    #[tokio::test]
    async fn test_mock_policy_deny() {
        let runtime = MockPolicyRuntime;
        let input = json!({
            "mcp": {
                "method": "tools/call",
                "tool_name": "shell.run"
            }
        });

        let decision = runtime.evaluate(std::sync::Arc::new(input)).await.unwrap(); //
        assert!(!decision.allow);
        assert_eq!(decision.reason, "tool is not allowed");
    }
}
