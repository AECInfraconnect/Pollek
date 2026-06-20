use crate::model::*;

pub fn infer_agent_type_from_name(name: &str) -> InferredAgentType {
    let lower = name.to_ascii_lowercase();
    if lower.contains("claude") {
        InferredAgentType::DesktopAgent
    } else if lower.contains("cursor") || lower.contains("code") || lower.contains("windsurf") {
        InferredAgentType::IdeAgent
    } else if lower.contains("ollama") || lower.contains("lmstudio") {
        InferredAgentType::LocalModelServer
    } else {
        InferredAgentType::UnknownAiProcess
    }
}

pub fn fingerprint_process(process_name: &str) -> f64 {
    let lower = process_name.to_ascii_lowercase();
    if lower.contains("claude") || lower.contains("cursor") || lower.contains("windsurf") {
        0.85
    } else if lower.contains("ollama") {
        0.95
    } else if lower.contains("code") {
        0.65
    } else {
        0.30
    }
}
