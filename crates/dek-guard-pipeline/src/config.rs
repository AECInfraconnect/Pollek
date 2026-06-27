// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GuardMode {
    Observe,
    Warn,
    #[default]
    Enforce,
    StrictDeny,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GuardThresholds {
    pub injection_warn_score: f32,
    pub injection_deny_score: f32,
    pub pii_confidence: f32,
}

impl Default for GuardThresholds {
    fn default() -> Self {
        Self {
            injection_warn_score: 0.45,
            injection_deny_score: 0.75,
            pii_confidence: 0.80,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GuardConfig {
    pub mode: GuardMode,
    pub request_guard_enabled: bool,
    pub response_guard_enabled: bool,
    pub telemetry_enabled: bool,
    pub enable_classifier: bool,
    pub enable_spotlight: bool,
    pub thresholds: GuardThresholds,
}

impl Default for GuardConfig {
    fn default() -> Self {
        Self {
            mode: GuardMode::Enforce,
            request_guard_enabled: true,
            response_guard_enabled: true,
            telemetry_enabled: true,
            enable_classifier: false,
            enable_spotlight: true,
            thresholds: GuardThresholds::default(),
        }
    }
}
