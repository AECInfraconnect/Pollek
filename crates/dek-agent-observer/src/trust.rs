use crate::model::AgentObservationEvent;
use std::collections::HashMap;

/// baseline พฤติกรรมปกติต่อ agent (สร้างจาก observation ย้อนหลัง)
#[derive(Debug, Clone, Default)]
pub struct AgentBaseline {
    pub typical_tools: HashMap<String, u64>, // tool -> ความถี่
    pub total_events: u64,
    pub deny_count: u64,
}

#[derive(Debug, Clone)]
pub struct TrustScore {
    pub agent_id: String,
    pub score: f64, // 0.0 (rogue) .. 1.0 (trusted)
    pub reasons: Vec<String>,
}

impl AgentBaseline {
    pub fn observe(&mut self, ev: &AgentObservationEvent) {
        self.total_events += 1;
        // Parse payload_json to find decision
        if let Ok(payload) = serde_json::from_str::<serde_json::Value>(&ev.payload_json) {
            if let Some(decision) = payload.get("decision").and_then(|v| v.as_str()) {
                if decision == "deny" {
                    self.deny_count += 1;
                }
            }
        }
        if let Some(tool) = &ev.tool_id {
            *self.typical_tools.entry(tool.clone()).or_insert(0) += 1;
        }
    }

    pub fn calculate_trust(&self, agent_id: &str) -> TrustScore {
        let mut score: f64 = 1.0;
        let mut reasons = Vec::new();
        if self.total_events > 0 {
            let deny_rate = self.deny_count as f64 / self.total_events as f64;
            if deny_rate > 0.1 {
                score -= 0.5;
                reasons.push(format!("High deny rate: {:.0}%", deny_rate * 100.0));
            }
        }
        TrustScore {
            agent_id: agent_id.to_string(),
            score: score.max(0.0),
            reasons,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum TrustAction {
    Normal,
    RequireApproval,
    KillSwitch,
}

pub fn enforce_trust(score: &TrustScore) -> TrustAction {
    if score.score < 0.3 {
        TrustAction::KillSwitch
    } else if score.score < 0.8 {
        TrustAction::RequireApproval
    } else {
        TrustAction::Normal
    }
}
