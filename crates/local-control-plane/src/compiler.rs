use anyhow::{anyhow, Result};
use dek_control_plane_api::policy::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompilationResult {
    pub success: bool,
    pub bytecode: Option<Vec<u8>>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    pub allowed: bool,
    pub evaluation_time_ms: u64,
    pub log_output: Vec<String>,
}

#[async_trait::async_trait]
pub trait PolicyCompiler: Send + Sync {
    async fn validate(&self, draft: &PolicyDraft) -> Result<ValidationResult>;
    async fn compile(&self, draft: &PolicyDraft) -> Result<CompilationResult>;
    async fn simulate(
        &self,
        draft: &PolicyDraft,
        input: serde_json::Value,
    ) -> Result<SimulationResult>;
}

pub struct RegoCompiler;

#[async_trait::async_trait]
impl PolicyCompiler for RegoCompiler {
    async fn validate(&self, draft: &PolicyDraft) -> Result<ValidationResult> {
        validate_raw_language(draft, "rego")
    }

    async fn compile(&self, draft: &PolicyDraft) -> Result<CompilationResult> {
        compile_raw_text(draft, "rego")
    }

    async fn simulate(
        &self,
        draft: &PolicyDraft,
        _input: serde_json::Value,
    ) -> Result<SimulationResult> {
        let start = std::time::Instant::now();
        let validation = validate_raw_language(draft, "rego")?;
        Ok(SimulationResult {
            allowed: false,
            evaluation_time_ms: start.elapsed().as_millis() as u64,
            log_output: if validation.is_valid {
                vec!["Rego local simulation is not available; fail closed.".to_string()]
            } else {
                validation.errors
            },
        })
    }
}

pub struct CedarCompiler;

#[async_trait::async_trait]
impl PolicyCompiler for CedarCompiler {
    async fn validate(&self, draft: &PolicyDraft) -> Result<ValidationResult> {
        validate_raw_language(draft, "cedar")
    }

    async fn compile(&self, draft: &PolicyDraft) -> Result<CompilationResult> {
        compile_raw_text(draft, "cedar")
    }

    async fn simulate(
        &self,
        draft: &PolicyDraft,
        input: serde_json::Value,
    ) -> Result<SimulationResult> {
        let start = std::time::Instant::now();
        let source = raw_text_for_language(draft, "cedar")?;
        let adapter = dek_cedar::CedarAdapter::new(&source)?;
        use dek_plugin_sdk::PolicyEvaluator;
        let request = dek_plugin_sdk::EvalRequest {
            request_id: format!("sim_{}", uuid::Uuid::new_v4()),
            tenant_id: Some(draft.meta.tenant_id.clone()),
            subject: None,
            action: None,
            resource: None,
            payload: input,
            context: std::collections::BTreeMap::new(),
        };
        let response = adapter.evaluate(request).await?;
        Ok(SimulationResult {
            allowed: response.decision == dek_plugin_sdk::DecisionEffect::Allow,
            evaluation_time_ms: start.elapsed().as_millis() as u64,
            log_output: vec![response.reason],
        })
    }
}

pub struct OpenFgaCompiler;

#[async_trait::async_trait]
impl PolicyCompiler for OpenFgaCompiler {
    async fn validate(&self, draft: &PolicyDraft) -> Result<ValidationResult> {
        validate_raw_language(draft, "openfga")
    }

    async fn compile(&self, draft: &PolicyDraft) -> Result<CompilationResult> {
        compile_raw_text(draft, "openfga")
    }

    async fn simulate(
        &self,
        draft: &PolicyDraft,
        _input: serde_json::Value,
    ) -> Result<SimulationResult> {
        let start = std::time::Instant::now();
        let validation = validate_raw_language(draft, "openfga")?;
        Ok(SimulationResult {
            allowed: false,
            evaluation_time_ms: start.elapsed().as_millis() as u64,
            log_output: if validation.is_valid {
                vec![
                    "OpenFGA local simulation requires a configured OpenFGA runtime; fail closed."
                        .to_string(),
                ]
            } else {
                validation.errors
            },
        })
    }
}

pub struct CompositePolicyCompiler;

#[async_trait::async_trait]
impl PolicyCompiler for CompositePolicyCompiler {
    async fn validate(&self, draft: &PolicyDraft) -> Result<ValidationResult> {
        match &draft.source {
            PolicySource::Structured { ir } => {
                match serde_json::from_value::<dek_policy_intent::PolicyIntent>(ir.clone()) {
                    Ok(intent) => match dek_policy_compiler::CompilerOrchestrator::compile(&intent)
                    {
                        Ok(_) => Ok(ValidationResult {
                            is_valid: true,
                            errors: Vec::new(),
                        }),
                        Err(error) => Ok(ValidationResult {
                            is_valid: false,
                            errors: vec![error.to_string()],
                        }),
                    },
                    Err(error) => Ok(ValidationResult {
                        is_valid: false,
                        errors: vec![format!("invalid structured policy intent: {error}")],
                    }),
                }
            }
            _ => Ok(ValidationResult {
                is_valid: false,
                errors: vec!["composite compiler requires structured policy intent".to_string()],
            }),
        }
    }

    async fn compile(&self, draft: &PolicyDraft) -> Result<CompilationResult> {
        let PolicySource::Structured { ir } = &draft.source else {
            return Ok(CompilationResult {
                success: false,
                bytecode: None,
                errors: vec!["composite compiler requires structured policy intent".to_string()],
            });
        };
        let intent = serde_json::from_value::<dek_policy_intent::PolicyIntent>(ir.clone())?;
        match dek_policy_compiler::CompilerOrchestrator::compile(&intent) {
            Ok(compiled) => Ok(CompilationResult {
                success: true,
                bytecode: Some(compiled.compiled_bytes),
                errors: Vec::new(),
            }),
            Err(error) => Ok(CompilationResult {
                success: false,
                bytecode: None,
                errors: vec![error.to_string()],
            }),
        }
    }

    async fn simulate(
        &self,
        draft: &PolicyDraft,
        _input: serde_json::Value,
    ) -> Result<SimulationResult> {
        let start = std::time::Instant::now();
        let validation = self.validate(draft).await?;
        Ok(SimulationResult {
            allowed: false,
            evaluation_time_ms: start.elapsed().as_millis() as u64,
            log_output: if validation.is_valid {
                vec!["Composite policy simulation must run through the selected PDP route; fail closed.".to_string()]
            } else {
                validation.errors
            },
        })
    }
}

fn validate_raw_language(draft: &PolicyDraft, expected_language: &str) -> Result<ValidationResult> {
    match raw_text_for_language(draft, expected_language) {
        Ok(text) => validate_raw_text(expected_language, &text),
        Err(error) => Ok(ValidationResult {
            is_valid: false,
            errors: vec![error.to_string()],
        }),
    }
}

fn compile_raw_text(draft: &PolicyDraft, expected_language: &str) -> Result<CompilationResult> {
    let text = raw_text_for_language(draft, expected_language)?;
    let validation = validate_raw_text(expected_language, &text)?;
    if !validation.is_valid {
        return Ok(CompilationResult {
            success: false,
            bytecode: None,
            errors: validation.errors,
        });
    }
    Ok(CompilationResult {
        success: true,
        bytecode: Some(text.into_bytes()),
        errors: Vec::new(),
    })
}

fn raw_text_for_language(draft: &PolicyDraft, expected_language: &str) -> Result<String> {
    let PolicySource::RawText { language, text } = &draft.source else {
        return Err(anyhow!(
            "{expected_language} compiler requires raw text policy source"
        ));
    };
    if !language_matches(language, expected_language) {
        return Err(anyhow!(
            "{expected_language} compiler cannot compile language {language}"
        ));
    }
    Ok(text.clone())
}

fn language_matches(language: &str, expected_language: &str) -> bool {
    let normalized = language.replace(['_', '-'], "").to_ascii_lowercase();
    let expected = expected_language
        .replace(['_', '-'], "")
        .to_ascii_lowercase();
    normalized == expected
}

fn validate_raw_text(language: &str, text: &str) -> Result<ValidationResult> {
    let errors = match language {
        "rego" => {
            if text.contains("package ") {
                Vec::new()
            } else {
                vec!["Rego policy is missing a package declaration".to_string()]
            }
        }
        "cedar" => match dek_cedar::CedarAdapter::new(text) {
            Ok(_) => Vec::new(),
            Err(error) => vec![error.to_string()],
        },
        "openfga" => {
            if text.contains("model") && text.contains("type ") {
                Vec::new()
            } else {
                vec!["OpenFGA model is missing model or type declarations".to_string()]
            }
        }
        other => vec![format!("unsupported policy language: {other}")],
    };
    Ok(ValidationResult {
        is_valid: errors.is_empty(),
        errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dek_control_plane_api::registry::{ObjectMeta, RegistrationSource, RegistryStatus};

    fn draft(language: &str, text: &str) -> PolicyDraft {
        PolicyDraft {
            meta: ObjectMeta {
                schema_version: "registry-object.v1".to_string(),
                tenant_id: "local".to_string(),
                workspace_id: "default".to_string(),
                environment_id: "dev".to_string(),
                created_at: "2026-06-27T00:00:00Z".to_string(),
                updated_at: "2026-06-27T00:00:00Z".to_string(),
                created_by: "test".to_string(),
                updated_by: "test".to_string(),
                source: RegistrationSource::Manual,
                status: RegistryStatus::Draft,
                tags: Vec::new(),
            },
            policy_id: "policy-test".to_string(),
            name: "Policy Test".to_string(),
            description: None,
            policy_type: PolicyType::Rego,
            targets: PolicyTargets {
                agent_ids: Vec::new(),
                tool_ids: Vec::new(),
                resource_ids: Vec::new(),
                entity_ids: Vec::new(),
                route_ids: Vec::new(),
            },
            source: PolicySource::RawText {
                language: language.to_string(),
                text: text.to_string(),
            },
            compile_options: PolicyCompileOptions {
                optimization_level: None,
                fail_on_warnings: Some(true),
            },
        }
    }

    #[tokio::test]
    async fn rego_compiler_returns_source_bytes_not_mock_bytecode() {
        let compiler = RegoCompiler;
        let source = "package policy\n\ndefault allow = false\n";
        let result = compiler.compile(&draft("rego", source)).await;

        assert!(result.is_ok());
        let Ok(result) = result else {
            return;
        };
        assert!(result.success);
        assert_eq!(result.bytecode, Some(source.as_bytes().to_vec()));
    }

    #[tokio::test]
    async fn rego_simulation_fails_closed_when_runtime_is_not_available() {
        let compiler = RegoCompiler;
        let result = compiler
            .simulate(&draft("rego", "package policy\n"), serde_json::json!({}))
            .await;

        assert!(result.is_ok());
        let Ok(result) = result else {
            return;
        };
        assert!(!result.allowed);
        assert!(result
            .log_output
            .iter()
            .any(|line| line.contains("fail closed")));
    }
}
