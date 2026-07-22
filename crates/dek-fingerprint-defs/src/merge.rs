use crate::model::*;
use std::collections::HashMap;

pub struct FingerprintDb {
    pub version: u64,
    pub by_id: HashMap<String, AgentSignatureV2>,
    pub web_ai: HashMap<String, WebAiSignatureDef>,
    pub installed_apps: HashMap<String, InstalledAppSignatureDef>,
    pub browser_processes: Vec<BrowserProcessDef>,
    pub ai_process_hints: AiProcessHints,
    pub cloud_resource_signatures: HashMap<String, CloudResourceSignatureDef>,
    pub collapse_rules: Vec<CollapseRuleDef>,
}

impl FingerprintDb {
    pub fn from_baseline(base: FingerprintDefinition) -> Self {
        let by_id = base
            .signatures
            .into_iter()
            .map(|s| (s.id.clone(), s))
            .collect();
        let web_ai = base
            .web_ai_signatures
            .into_iter()
            .map(|s| (s.stable_id().to_string(), s))
            .collect();
        let installed_apps = base
            .installed_app_signatures
            .into_iter()
            .map(|s| (s.id.clone(), s))
            .collect();
        Self {
            version: base.definition_version,
            by_id,
            web_ai,
            installed_apps,
            browser_processes: base.browser_processes,
            ai_process_hints: base.ai_process_hints,
            cloud_resource_signatures: base
                .cloud_resource_signatures
                .into_iter()
                .map(|s| (s.name.clone(), s))
                .collect(),
            collapse_rules: base.collapse_rules,
        }
    }

    pub fn apply(&mut self, def: FingerprintDefinition) -> anyhow::Result<()> {
        match def.kind {
            DefinitionKind::Full => {
                self.by_id = def
                    .signatures
                    .into_iter()
                    .map(|s| (s.id.clone(), s))
                    .collect();
                self.web_ai = def
                    .web_ai_signatures
                    .into_iter()
                    .map(|s| (s.stable_id().to_string(), s))
                    .collect();
                self.installed_apps = def
                    .installed_app_signatures
                    .into_iter()
                    .map(|s| (s.id.clone(), s))
                    .collect();
                self.browser_processes = def.browser_processes;
                self.ai_process_hints = def.ai_process_hints;
                self.cloud_resource_signatures = def
                    .cloud_resource_signatures
                    .into_iter()
                    .map(|s| (s.name.clone(), s))
                    .collect();
                self.collapse_rules = def.collapse_rules;
            }
            DefinitionKind::Delta => {
                if def.base_version != Some(self.version) {
                    anyhow::bail!(
                        "delta base {:?} != current {} — a full definition sync is required",
                        def.base_version,
                        self.version
                    );
                }
                for sig in def.signatures {
                    self.by_id.insert(sig.id.clone(), sig);
                }
                for sig in def.web_ai_signatures {
                    self.web_ai.insert(sig.stable_id().to_string(), sig);
                }
                for sig in def.installed_app_signatures {
                    self.installed_apps.insert(sig.id.clone(), sig);
                }
                for sig in def.cloud_resource_signatures {
                    self.cloud_resource_signatures.insert(sig.name.clone(), sig);
                }
                for browser in def.browser_processes {
                    upsert_browser_process(&mut self.browser_processes, browser);
                }
                for rule in def.collapse_rules {
                    upsert_collapse_rule(&mut self.collapse_rules, rule);
                }
                if !def.ai_process_hints.name_tokens.is_empty()
                    || !def.ai_process_hints.cmd_tokens.is_empty()
                    || !def.ai_process_hints.deny_tokens.is_empty()
                    || def.ai_process_hints.require_match
                {
                    self.ai_process_hints = def.ai_process_hints;
                }
                for id in &def.removed_ids {
                    self.by_id.remove(id);
                    self.web_ai.remove(id);
                    self.installed_apps.remove(id);
                    self.cloud_resource_signatures.remove(id);
                    self.collapse_rules.retain(|rule| rule.id != *id);
                    self.browser_processes.retain(|b| {
                        !b.process_names
                            .iter()
                            .any(|name| name.eq_ignore_ascii_case(id))
                    });
                }
            }
        }
        self.version = def.definition_version;
        Ok(())
    }
}

fn upsert_collapse_rule(items: &mut Vec<CollapseRuleDef>, incoming: CollapseRuleDef) {
    if let Some(existing) = items.iter_mut().find(|rule| rule.id == incoming.id) {
        *existing = incoming;
    } else {
        items.push(incoming);
    }
}

fn upsert_browser_process(items: &mut Vec<BrowserProcessDef>, incoming: BrowserProcessDef) {
    if let Some(existing) = items.iter_mut().find(|existing| {
        existing.engine == incoming.engine
            && existing.process_names.iter().any(|existing_name| {
                incoming
                    .process_names
                    .iter()
                    .any(|incoming_name| existing_name.eq_ignore_ascii_case(incoming_name))
            })
    }) {
        for name in incoming.process_names {
            if !existing
                .process_names
                .iter()
                .any(|n| n.eq_ignore_ascii_case(&name))
            {
                existing.process_names.push(name);
            }
        }
    } else {
        items.push(incoming);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sig(id: &str, rev: u32) -> AgentSignatureV2 {
        AgentSignatureV2 {
            id: id.into(),
            display_name: id.into(),
            agent_type: "cli_agent".into(),
            revision: rev,
            meta: SignatureMeta {
                author: "t".into(),
                description: "".into(),
                references: vec![],
                added_in: "1".into(),
                tags: vec![],
            },
            process_names: vec![],
            binary_hashes: vec![],
            config_paths: Default::default(),
            config_parsers: vec![],
            ports: vec![],
            port_probe: None,
            detection_logic: DetectionLogic::AnyOf,
            control_strategies: vec![],
            risk_weight: 0.5,
            cmd_patterns: vec![],
            exe_path_patterns: vec![],
            install_markers: vec![],
            cli_binaries: vec![],
            package_markers: vec![],
            env_markers: vec![],
            egress_hosts: vec![],
            vendor: None,
            product: None,
            capability_tags: vec![],
            signal_weights: None,
        }
    }

    #[test]
    fn delta_adds_and_removes() -> anyhow::Result<()> {
        let base = FingerprintDefinition {
            schema_version: "v2".into(),
            definition_version: 1,
            released_at: "".into(),
            min_engine_version: "1.0.0".into(),
            kind: DefinitionKind::Full,
            base_version: None,
            signatures: vec![sig("ollama", 1)],
            removed_ids: vec![],
            catalog_hash: "".into(),
            model_classifier: None,
            web_ai_signatures: vec![],
            installed_app_signatures: vec![],
            browser_processes: vec![],
            ai_process_hints: AiProcessHints::default(),
            cloud_resource_signatures: vec![],
            collapse_rules: vec![],
        };
        let mut db = FingerprintDb::from_baseline(base);
        let delta = FingerprintDefinition {
            schema_version: "v2".into(),
            definition_version: 2,
            released_at: "".into(),
            min_engine_version: "1.0.0".into(),
            kind: DefinitionKind::Delta,
            base_version: Some(1),
            signatures: vec![sig("goose_cli", 1)],
            removed_ids: vec!["ollama".into()],
            catalog_hash: "".into(),
            model_classifier: None,
            web_ai_signatures: vec![],
            installed_app_signatures: vec![],
            browser_processes: vec![],
            ai_process_hints: AiProcessHints::default(),
            cloud_resource_signatures: vec![],
            collapse_rules: vec![],
        };
        db.apply(delta)?;
        assert!(db.by_id.contains_key("goose_cli"));
        assert!(!db.by_id.contains_key("ollama"));
        assert_eq!(db.version, 2);
        Ok(())
    }

    #[test]
    fn delta_rejects_wrong_base() {
        let mut db = FingerprintDb {
            version: 1,
            by_id: std::collections::HashMap::new(),
            web_ai: std::collections::HashMap::new(),
            installed_apps: std::collections::HashMap::new(),
            browser_processes: vec![],
            ai_process_hints: AiProcessHints::default(),
            cloud_resource_signatures: std::collections::HashMap::new(),
            collapse_rules: vec![],
        };
        let bad = FingerprintDefinition {
            schema_version: "v2".into(),
            definition_version: 7,
            released_at: "".into(),
            min_engine_version: "1.0.0".into(),
            kind: DefinitionKind::Delta,
            base_version: Some(3),
            signatures: vec![],
            removed_ids: vec![],
            catalog_hash: "bad".into(),
            model_classifier: None,
            web_ai_signatures: vec![],
            installed_app_signatures: vec![],
            browser_processes: vec![],
            ai_process_hints: AiProcessHints::default(),
            cloud_resource_signatures: vec![],
            collapse_rules: vec![],
        };
        assert!(db.apply(bad).is_err());
    }
}
