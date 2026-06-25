// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::deployment_session::LocalizedText;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Agent,
    User,
    Device,
    McpServer,
    Tool,
    Resource,
    Policy,
    Deployment,
    CapabilitySnapshot,
    ControlMethod,
    Evidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EntityStatus {
    Ready,
    Active,
    ObserveOnly,
    NeedsApproval,
    NeedsSetup,
    Partial,
    Warning,
    Failed,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChipTone {
    Neutral,
    Success,
    Warning,
    Danger,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EntityChip {
    pub label: String,
    pub tone: ChipTone,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EntityMetric {
    pub label: String,
    // Value could be string or number, keep as string in Rust for simplicity or use untagged enum
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EntityCardModel {
    pub id: String,
    pub kind: EntityKind,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    pub status: EntityStatus,
    pub status_label: LocalizedText,
    pub summary: LocalizedText,
    #[serde(default)]
    pub chips: Vec<EntityChip>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<Vec<EntityMetric>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated_at: Option<DateTime<Utc>>,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn test_entity_card_serde() {
        let json = r#"{"id":"1","kind":"agent","title":"Test Agent","status":"ready","status_label":{"en":"Ready","th":"�������ҹ"},"summary":{"en":"Summary","th":"��ػ"},"chips":[]}"#;
        let card: EntityCardModel = serde_json::from_str(json).unwrap();
        assert_eq!(card.kind, EntityKind::Agent);
    }
}
