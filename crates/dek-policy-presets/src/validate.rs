// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use crate::model::{ControlMode, DeployPresetRequest, PepType, PolicyPresetV2, PresetValueType};
use std::collections::BTreeMap;

pub fn validate_params(
    preset: &PolicyPresetV2,
    params: &BTreeMap<String, serde_json::Value>,
) -> anyhow::Result<()> {
    for param_def in &preset.parameters {
        if param_def.required && !params.contains_key(&param_def.key) {
            anyhow::bail!("Missing required parameter: {}", param_def.key);
        }

        if let Some(val) = params.get(&param_def.key) {
            match param_def.value_type {
                PresetValueType::String if !val.is_string() => {
                    anyhow::bail!("Parameter '{}' must be a string", param_def.key);
                }
                PresetValueType::Integer if !val.is_i64() => {
                    anyhow::bail!("Parameter '{}' must be an integer", param_def.key);
                }
                PresetValueType::Float if !val.is_f64() && !val.is_i64() => {
                    anyhow::bail!("Parameter '{}' must be a float", param_def.key);
                }
                PresetValueType::Boolean if !val.is_boolean() => {
                    anyhow::bail!("Parameter '{}' must be a boolean", param_def.key);
                }
                PresetValueType::StringList
                | PresetValueType::PathList
                | PresetValueType::GlobList
                | PresetValueType::ProviderList
                | PresetValueType::AgentSelector
                | PresetValueType::ToolSelector
                | PresetValueType::ResourceSelector if !val.is_array() => {
                    anyhow::bail!("Parameter '{}' must be an array", param_def.key);
                }
                _ => {} // Other validations can be added later
            }
        }
    }
    Ok(())
}

pub fn validate_pep_selection(
    preset: &PolicyPresetV2,
    selected_peps: &[PepType],
    control_mode: &ControlMode,
) -> anyhow::Result<()> {
    if selected_peps.is_empty() {
        anyhow::bail!("At least one PEP must be selected");
    }

    if !preset.supported_control_modes.contains(control_mode) {
        anyhow::bail!(
            "Control mode '{:?}' is not supported by this preset",
            control_mode
        );
    }

    for pep in selected_peps {
        if !preset.supported_pep_types.contains(pep) {
            anyhow::bail!("PEP type '{:?}' is not supported by this preset", pep);
        }
    }

    Ok(())
}

pub fn validate_request(preset: &PolicyPresetV2, req: &DeployPresetRequest) -> anyhow::Result<()> {
    validate_params(preset, &req.params)?;
    validate_pep_selection(preset, &req.selected_pep_types, &req.control_mode)?;
    Ok(())
}
