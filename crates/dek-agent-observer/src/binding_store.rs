use anyhow::Result;
use dek_agent_binding::binding::AgentBinding;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

pub struct AgentBindingStore {
    conn: Mutex<Connection>,
}

impl AgentBindingStore {
    pub fn new() -> Result<Self> {
        let db_path = dirs::data_local_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("pollen_dek")
            .join("binding_store.db");

        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&db_path)?;
        Self::init_db(&conn)?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::init_db(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn init_db(conn: &Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS bindings (
                binding_id TEXT PRIMARY KEY,
                signature_id TEXT NOT NULL,
                data TEXT NOT NULL
            )",
            [],
        )?;
        Ok(())
    }

    pub fn save(&self, binding: AgentBinding) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;
        let data = serde_json::to_string(&binding)?;
        conn.execute(
            "INSERT INTO bindings (binding_id, signature_id, data) VALUES (?1, ?2, ?3)
             ON CONFLICT(binding_id) DO UPDATE SET data=excluded.data, signature_id=excluded.signature_id",
            (&binding.binding_id, &binding.signature_id, &data),
        )?;
        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<Option<AgentBinding>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;
        let mut stmt = conn.prepare("SELECT data FROM bindings WHERE binding_id = ?1")?;
        let mut rows = stmt.query([id])?;
        if let Some(row) = rows.next()? {
            let data: String = row.get(0)?;
            let binding: AgentBinding = serde_json::from_str(&data)?;
            Ok(Some(binding))
        } else {
            Ok(None)
        }
    }

    pub fn list_all(&self) -> Result<Vec<AgentBinding>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;
        let mut stmt = conn.prepare("SELECT data FROM bindings")?;
        let rows = stmt.query_map([], |row| {
            let data: String = row.get(0)?;
            Ok(data)
        })?;

        let mut bindings = Vec::new();
        for row in rows {
            let data = row?;
            if let Ok(b) = serde_json::from_str(&data) {
                bindings.push(b);
            }
        }
        Ok(bindings)
    }

    pub fn get_by_signature(&self, signature_id: &str) -> Result<Vec<AgentBinding>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;
        let mut stmt = conn.prepare("SELECT data FROM bindings WHERE signature_id = ?1")?;
        let rows = stmt.query_map([signature_id], |row| {
            let data: String = row.get(0)?;
            Ok(data)
        })?;

        let mut bindings = Vec::new();
        for row in rows {
            let data = row?;
            if let Ok(b) = serde_json::from_str(&data) {
                bindings.push(b);
            }
        }
        Ok(bindings)
    }
}



pub type SharedBindingStore = Arc<AgentBindingStore>;
