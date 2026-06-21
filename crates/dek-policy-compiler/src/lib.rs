use dek_policy_intent::PolicyIntent;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompilerError {
    #[error("Missing preferred PEP types in policy intent")]
    MissingPepType,
    #[error("Unsupported PEP type: {0}")]
    UnsupportedPepType(String),
    #[error("Compilation failed: {0}")]
    CompilationFailed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledPolicy {
    pub engine: String,
    pub source: String,
    pub compiled_bytes: Vec<u8>,
}

pub struct CompilerOrchestrator;

impl CompilerOrchestrator {
    pub fn compile(intent: &PolicyIntent) -> Result<CompiledPolicy, CompilerError> {
        if intent.spec.enforcement.preferred_pep_types.is_empty() {
            return Err(CompilerError::MissingPepType);
        }

        let pep_type = &intent.spec.enforcement.preferred_pep_types[0];

        match pep_type.as_str() {
            "mcp_proxy" | "cedar" => Self::compile_to_cedar(intent),
            "opa_wasm" | "opa" => Self::compile_to_opa(intent),
            _ => Err(CompilerError::UnsupportedPepType(pep_type.clone())),
        }
    }

    fn compile_to_cedar(intent: &PolicyIntent) -> Result<CompiledPolicy, CompilerError> {
        let effect = match intent.spec.decision_mode {
            dek_policy_intent::DecisionMode::Enforce => "permit",
            _ => "permit",
        };

        // Very basic stub compiler for Cedar
        let mut source = format!("// Generated from {}\n", intent.metadata.name);
        source.push_str(&format!("{}(\n", effect));
        source.push_str("  principal,\n");
        source.push_str("  action,\n");
        source.push_str("  resource\n");
        source.push_str(");\n");

        Ok(CompiledPolicy {
            engine: "cedar".to_string(),
            source: source.clone(),
            compiled_bytes: source.into_bytes(),
        })
    }

    fn compile_to_opa(intent: &PolicyIntent) -> Result<CompiledPolicy, CompilerError> {
        let source = format!("package policy\ndefault allow = false\nallow {{\n  # Generated from {}\n  input.action != \"\"\n}}\n", intent.metadata.name);
        Ok(CompiledPolicy {
            engine: "opa_wasm".to_string(),
            source: source.clone(),
            compiled_bytes: source.into_bytes(),
        })
    }
}
