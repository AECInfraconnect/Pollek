// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use axum::{
    extract::{Path, State},
    routing::post,
    Json, Router,
};
use dek_domain_schema::Policy;
use serde::{Deserialize, Serialize};

use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompilerTarget {
    Rego,
    Cedar,
    Openfga,
    Wasm,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileRequest {
    pub target: CompilerTarget,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledPolicy {
    pub target: String,
    pub source_code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compiled_binary_b64: Option<String>,
}

pub fn compile_policy(policy: &Policy, target: CompilerTarget) -> Result<CompiledPolicy, String> {
    match target {
        CompilerTarget::Rego => Ok(CompiledPolicy {
            target: "rego".to_string(),
            source_code: format!(
                "package pollek.authz\n\ndefault allow = false\n\n# Simulated Rego for Policy {}\nallow {{\n    input.action == \"*\"\n}}",
                policy.policy_id
            ),
            compiled_binary_b64: None,
        }),
        CompilerTarget::Cedar => Ok(CompiledPolicy {
            target: "cedar".to_string(),
            source_code: format!(
                "permit(\n  principal,\n  action,\n  resource\n) when {{ /* simulated for {} */ }};",
                policy.policy_id
            ),
            compiled_binary_b64: None,
        }),
        CompilerTarget::Openfga => Ok(CompiledPolicy {
            target: "openfga".to_string(),
            source_code: "model\n  schema 1.1\ntype user\ntype resource\n  relations\n    define viewer: [user]".to_string(),
            compiled_binary_b64: None,
        }),
        CompilerTarget::Wasm => {
            // Fake WASM magic header: \0asm
            use base64::{engine::general_purpose::STANDARD, Engine as _};
            let binary = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
            Ok(CompiledPolicy {
                target: "wasm".to_string(),
                source_code: "// Rust source code converted to WASM".to_string(),
                compiled_binary_b64: Some(STANDARD.encode(binary)),
            })
        }
    }
}

pub async fn compile_policy_handler(
    State(state): State<AppState>,
    Path((tenant_id, policy_id)): Path<(String, String)>,
    Json(payload): Json<CompileRequest>,
) -> axum::response::Result<Json<CompiledPolicy>, axum::http::StatusCode> {
    let registry = state.registry.lock().unwrap(); //
    let policy = registry
        .policies
        .get(&policy_id)
        .filter(|p| p.tenant_id == tenant_id)
        .ok_or(axum::http::StatusCode::NOT_FOUND)?;

    let compiled = compile_policy(policy, payload.target)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(compiled))
}

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/v1/tenants/:tenant_id/policies/:policy_id/compile",
        post(compile_policy_handler),
    )
}
