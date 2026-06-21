#![no_main]
use libfuzzer_sys::fuzz_target;
use dek_policy_presets::model::{PresetApplyRequest, PolicyPreset, PresetCategory, PresetLanguage, ControlLevel, PresetTemplate};

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(s) {
            let preset = PolicyPreset {
                preset_id: "fuzz".into(),
                version: "1".into(),
                display_name: "fuzz".into(),
                description: "fuzz".into(),
                category: PresetCategory::ShadowAi,
                language: PresetLanguage::Rego,
                recommended_pep_types: vec![],
                supported_control_levels: vec![],
                default_control_level: ControlLevel::ObserveOnly,
                risk_tags: vec![],
                owasp_tags: vec![],
                parameters: vec![],
                template: PresetTemplate {
                    source: "{}".into(),
                    entrypoint: None,
                },
                test_cases: vec![],
            };
            
            let req = PresetApplyRequest {
                targets: Default::default(),
                control_level: ControlLevel::ObserveOnly,
                params: std::collections::HashMap::new(),
            };
            
            // Fuzz passing malicious strings as param values
            if let Some(obj) = json.as_object() {
                let mut req = req;
                for (k, v) in obj {
                    req.params.insert(k.clone(), v.clone());
                }
                let _ = dek_policy_presets::render(&preset, &req);
            }
        }
    }
});
