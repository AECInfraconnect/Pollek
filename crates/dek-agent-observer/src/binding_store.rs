use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use dek_agent_binding::binding::AgentBinding;
use anyhow::Result;

pub struct AgentBindingStore {
    // In-memory store for now, typically backed by SQLite/sled in production
    bindings: RwLock<HashMap<String, AgentBinding>>,
}

impl AgentBindingStore {
    pub fn new() -> Self {
        Self {
            bindings: RwLock::new(HashMap::new()),
        }
    }

    pub fn save(&self, binding: AgentBinding) -> Result<()> {
        let mut w = self.bindings.write().map_err(|_| anyhow::anyhow!("lock error"))?;
        w.insert(binding.binding_id.clone(), binding);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<Option<AgentBinding>> {
        let r = self.bindings.read().map_err(|_| anyhow::anyhow!("lock error"))?;
        Ok(r.get(id).cloned())
    }

    pub fn list_all(&self) -> Result<Vec<AgentBinding>> {
        let r = self.bindings.read().map_err(|_| anyhow::anyhow!("lock error"))?;
        Ok(r.values().cloned().collect())
    }

    pub fn get_by_signature(&self, signature_id: &str) -> Result<Vec<AgentBinding>> {
        let r = self.bindings.read().map_err(|_| anyhow::anyhow!("lock error"))?;
        Ok(r.values()
            .filter(|b| b.signature_id == signature_id)
            .cloned()
            .collect())
    }
}

pub type SharedBindingStore = Arc<AgentBindingStore>;
