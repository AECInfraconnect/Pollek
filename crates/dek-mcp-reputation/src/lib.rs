// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationEntry {
    pub mcp_id: String,
    pub score: u32, // 0-100
    pub is_allowed: bool,
    pub description: String,
}

#[derive(Debug, Clone, Default)]
pub struct ReputationRegistry {
    entries: HashMap<String, ReputationEntry>,
}

impl ReputationRegistry {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn load_local(&mut self) -> Result<()> {
        let config_dir = dek_config::paths::get_config_dir();
        let path = config_dir.join("mcp_registry.json");
        if path.exists() {
            let data = std::fs::read_to_string(&path)?;
            let loaded: Vec<ReputationEntry> = serde_json::from_str(&data)?;
            for entry in loaded {
                self.entries.insert(entry.mcp_id.clone(), entry);
            }
        }
        Ok(())
    }

    pub fn lookup(&self, mcp_id: &str) -> Option<ReputationEntry> {
        self.entries.get(mcp_id).cloned()
    }

    pub fn save_local(&self) -> Result<()> {
        let config_dir = dek_config::paths::get_config_dir();
        let path = config_dir.join("mcp_registry.json");
        let list: Vec<&ReputationEntry> = self.entries.values().collect();
        let data = serde_json::to_string_pretty(&list)?;
        std::fs::create_dir_all(&config_dir).ok();
        std::fs::write(&path, data)?;
        Ok(())
    }

    pub fn add_entry(&mut self, entry: ReputationEntry) {
        self.entries.insert(entry.mcp_id.clone(), entry);
    }
}
