use crate::model::*;
use std::collections::HashMap;

pub struct FingerprintDb {
    pub version: u64,
    pub by_id: HashMap<String, AgentSignatureV2>,
}

impl FingerprintDb {
    pub fn from_baseline(base: FingerprintDefinition) -> Self {
        let by_id = base.signatures.into_iter().map(|s| (s.id.clone(), s)).collect();
        Self { version: base.definition_version, by_id }
    }

    pub fn apply(&mut self, def: FingerprintDefinition) -> anyhow::Result<()> {
        match def.kind {
            DefinitionKind::Full => {
                self.by_id = def.signatures.into_iter().map(|s| (s.id.clone(), s)).collect();
            }
            DefinitionKind::Delta => {
                if def.base_version != Some(self.version) {
                    anyhow::bail!(
                        "delta base {:?} != current {} — ต้องดึง full",
                        def.base_version, self.version
                    );
                }
                for sig in def.signatures {
                    self.by_id.insert(sig.id.clone(), sig);
                }
                for id in &def.removed_ids {
                    self.by_id.remove(id);
                }
            }
        }
        self.version = def.definition_version;
        Ok(())
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
        }
    }

    #[test]
    fn delta_adds_and_removes() {
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
        };
        db.apply(delta).unwrap();
        assert!(db.by_id.contains_key("goose_cli"));
        assert!(!db.by_id.contains_key("ollama"));
        assert_eq!(db.version, 2);
    }

    #[test]
    fn delta_rejects_wrong_base() {
        let mut db = FingerprintDb { version: 5, by_id: Default::default() };
        let bad = FingerprintDefinition {
            schema_version: "v2".into(),
            definition_version: 7,
            released_at: "".into(),
            min_engine_version: "1.0.0".into(),
            kind: DefinitionKind::Delta,
            base_version: Some(3),
            signatures: vec![],
            removed_ids: vec![],
            catalog_hash: "".into(),
        };
        assert!(db.apply(bad).is_err());
    }
}
