// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProfile {
    pub agent_id: String,
    pub permissions: Vec<String>,
    pub shadow_ai_status: bool,
    pub associated_mcps: Vec<McpProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpProfile {
    pub mcp_id: String,
    pub reputation_score: u32, // 0-100
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskScore {
    pub total_score: u32, // 0-100, higher is riskier
    pub factors: Vec<String>,
}

pub fn calculate_risk_score(profile: &AgentProfile) -> RiskScore {
    let mut score = 0;
    let mut factors = Vec::new();

    let mut registry = dek_mcp_reputation::ReputationRegistry::new();
    let _ = registry.load_local();

    // Shadow AI Check
    if profile.shadow_ai_status {
        score += 40;
        factors.push("Unmanaged Shadow AI detected".to_string());
    }

    // Over-permissions Check
    let sensitive_perms = ["filesystem:write", "network:outbound", "process:execute"];
    let mut sensitive_count = 0;
    for perm in &profile.permissions {
        if sensitive_perms.contains(&perm.as_str()) {
            sensitive_count += 1;
        }
    }

    if sensitive_count > 0 {
        score += sensitive_count * 10;
        factors.push(format!("Has {} sensitive permissions", sensitive_count));
    }

    // MCP Reputation Check using Registry
    for mcp in &profile.associated_mcps {
        if let Some(entry) = registry.lookup(&mcp.mcp_id) {
            if !entry.is_allowed {
                score += 50;
                factors.push(format!(
                    "Associated with explicitly denied MCP: {}",
                    mcp.mcp_id
                ));
            } else if entry.score < 50 {
                score += 20;
                factors.push(format!(
                    "Associated with low reputation MCP: {}",
                    mcp.mcp_id
                ));
            } else if entry.score < 80 {
                score += 5;
                factors.push(format!(
                    "Associated with medium reputation MCP: {}",
                    mcp.mcp_id
                ));
            }
        } else {
            // Unknown MCP
            score += 10;
            factors.push(format!(
                "Associated with unknown MCP (not in registry): {}",
                mcp.mcp_id
            ));
        }
    }

    if score > 100 {
        score = 100;
    }

    RiskScore {
        total_score: score,
        factors,
    }
}
