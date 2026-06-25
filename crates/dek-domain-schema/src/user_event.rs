// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::deployment_session::LocalizedText;
use crate::feasibility::RequiredUserAction;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AdvancedDiagnostic {
    pub raw_data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UserVisibleEvent {
    pub event_id: String,
    pub correlation_id: String,
    pub scan_id: Option<String>,
    pub deployment_id: Option<String>,
    pub agent_id: Option<String>,
    pub entity_id: Option<String>,
    pub policy_id: Option<String>,
    pub category: EventCategory,
    pub status: EventStatus,
    pub title: LocalizedText,
    pub detail: LocalizedText,
    pub next_action: Option<RequiredUserAction>,
    pub advanced: Option<AdvancedDiagnostic>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventCategory {
    Discovery,
    Capability,
    PolicyFeasibility,
    Deployment,
    Approval,
    Enforcement,
    Observation,
    Telemetry,
    Health,
    Rollback,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventStatus {
    Queued,
    Running,
    Succeeded,
    Warning,
    Failed,
    TimedOut,
    Skipped,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_user_event_serde() {
        let json = r#"{"event_id":"1","correlation_id":"2","category":"discovery","status":"succeeded","title":{"en":"T","th":"T"},"detail":{"en":"D","th":"D"},"created_at":"2026-06-25T00:00:00Z"}"#;
        let event: UserVisibleEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.category, EventCategory::Discovery);
    }
}
