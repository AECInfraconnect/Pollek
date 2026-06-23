#![allow(clippy::unwrap_used, clippy::needless_borrow)]
use anyhow::Result;
use dek_agent_observer::model::{AgentObservationEvent, CostLedgerEntry};
use dek_control_plane_api::registry::*;
use dek_policy_suggester::model::PolicySuggestion;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[async_trait::async_trait]
pub trait RegistryStore: Send + Sync {
    async fn upsert_agent(&self, agent: AiAgent) -> Result<AiAgent>;
    async fn get_agent(&self, tenant_id: &str, agent_id: &str) -> Result<Option<AiAgent>>;
    async fn list_agents(&self, tenant_id: &str) -> Result<Vec<AiAgent>>;
    async fn delete_agent(&self, tenant_id: &str, agent_id: &str) -> Result<bool>;

    async fn upsert_raw(
        &self,
        tenant_id: &str,
        object_type: &str,
        object_id: &str,
        data: &serde_json::Value,
    ) -> Result<()>;
    async fn get_raw(
        &self,
        tenant_id: &str,
        object_type: &str,
        object_id: &str,
    ) -> Result<Option<serde_json::Value>>;
    async fn list_raw(&self, tenant_id: &str, object_type: &str) -> Result<Vec<serde_json::Value>>;

    async fn upsert_blackbox_ai(&self, provider: BlackboxAiProvider) -> Result<BlackboxAiProvider>;
    async fn get_blackbox_ai(
        &self,
        tenant_id: &str,
        provider_id: &str,
    ) -> Result<Option<BlackboxAiProvider>>;
    async fn list_blackbox_ai(&self, tenant_id: &str) -> Result<Vec<BlackboxAiProvider>>;
    async fn delete_blackbox_ai(&self, tenant_id: &str, provider_id: &str) -> Result<bool>;

    async fn upsert_entity(&self, entity: Entity) -> Result<Entity>;
    async fn get_entity(&self, tenant_id: &str, entity_id: &str) -> Result<Option<Entity>>;
    async fn list_entities(&self, tenant_id: &str) -> Result<Vec<Entity>>;
    async fn delete_entity(&self, tenant_id: &str, entity_id: &str) -> Result<bool>;

    async fn upsert_resource(&self, resource: Resource) -> Result<Resource>;
    async fn get_resource(&self, tenant_id: &str, resource_id: &str) -> Result<Option<Resource>>;
    async fn list_resources(&self, tenant_id: &str) -> Result<Vec<Resource>>;
    async fn delete_resource(&self, tenant_id: &str, resource_id: &str) -> Result<bool>;

    async fn upsert_tool(&self, tool: Tool) -> Result<Tool>;
    async fn get_tool(&self, tenant_id: &str, tool_id: &str) -> Result<Option<Tool>>;
    async fn list_tools(&self, tenant_id: &str) -> Result<Vec<Tool>>;
    async fn delete_tool(&self, tenant_id: &str, tool_id: &str) -> Result<bool>;

    async fn upsert_mcp_server(&self, server: McpServer) -> Result<McpServer>;
    async fn get_mcp_server(&self, tenant_id: &str, server_id: &str) -> Result<Option<McpServer>>;
    async fn list_mcp_servers(&self, tenant_id: &str) -> Result<Vec<McpServer>>;
    async fn delete_mcp_server(&self, tenant_id: &str, server_id: &str) -> Result<bool>;

    async fn upsert_relationship(&self, relationship: Relationship) -> Result<Relationship>;
    async fn get_relationship(
        &self,
        tenant_id: &str,
        relationship_id: &str,
    ) -> Result<Option<Relationship>>;
    async fn list_relationships(&self, tenant_id: &str) -> Result<Vec<Relationship>>;
    async fn delete_relationship(&self, tenant_id: &str, relationship_id: &str) -> Result<bool>;

    async fn upsert_agent_inventory(
        &self,
        inventory: dek_domain_schema::AgentCapabilityInventory,
    ) -> Result<dek_domain_schema::AgentCapabilityInventory>;
    async fn get_agent_inventory(
        &self,
        tenant_id: &str,
        agent_id: &str,
    ) -> Result<Option<dek_domain_schema::AgentCapabilityInventory>>;
    async fn list_agent_inventories(
        &self,
        tenant_id: &str,
    ) -> Result<Vec<dek_domain_schema::AgentCapabilityInventory>>;
    async fn delete_agent_inventory(&self, tenant_id: &str, agent_id: &str) -> Result<bool>;
}

#[async_trait::async_trait]
pub trait PolicyStore: Send + Sync {
    async fn upsert_policy(
        &self,
        policy: dek_control_plane_api::policy::PolicyDraft,
    ) -> Result<dek_control_plane_api::policy::PolicyDraft>;
    async fn get_policy(
        &self,
        tenant_id: &str,
        policy_id: &str,
    ) -> Result<Option<dek_control_plane_api::policy::PolicyDraft>>;
    async fn list_policies(
        &self,
        tenant_id: &str,
    ) -> Result<Vec<dek_control_plane_api::policy::PolicyDraft>>;
    async fn delete_policy(&self, tenant_id: &str, policy_id: &str) -> Result<bool>;
    async fn put_policy_status(
        &self,
        tenant_id: &str,
        policy_id: &str,
        status: dek_control_plane_api::policy::PolicyLifecycleStatus,
    ) -> Result<()>;

    async fn upsert_policy_raw(
        &self,
        tenant: &str,
        id: &str,
        data: &serde_json::Value,
    ) -> Result<()>;
    async fn get_policy_raw(&self, tenant: &str, id: &str) -> Result<Option<serde_json::Value>>;
    async fn put_blob(&self, tenant: &str, path: &str, bytes: &[u8]) -> Result<()>;
    async fn get_blob(&self, tenant: &str, path: &str) -> Result<Option<Vec<u8>>>;

    async fn upsert_preset_deployment(
        &self,
        tenant_id: &str,
        deployment_id: &str,
        data: &serde_json::Value,
    ) -> Result<()>;
    async fn get_preset_deployment(
        &self,
        tenant_id: &str,
        deployment_id: &str,
    ) -> Result<Option<serde_json::Value>>;
    async fn list_preset_deployments(&self, tenant_id: &str) -> Result<Vec<serde_json::Value>>;

    async fn upsert_pep_binding(
        &self,
        tenant_id: &str,
        binding_id: &str,
        deployment_id: &str,
        pep_type: &str,
        data: &serde_json::Value,
    ) -> Result<()>;
    async fn list_pep_bindings(
        &self,
        tenant_id: &str,
        deployment_id: &str,
    ) -> Result<Vec<serde_json::Value>>;
}

#[async_trait::async_trait]
pub trait TelemetryStore: Send + Sync {
    async fn put_telemetry(
        &self,
        tenant: &str,
        kind: &str,
        event_id: &str,
        data: &serde_json::Value,
    ) -> Result<()>;
    async fn list_telemetry(&self, tenant: &str, kind: &str) -> Result<Vec<serde_json::Value>>;
}

#[async_trait::async_trait]
pub trait PdpStore: Send + Sync {
    async fn upsert_runtime(&self, tenant: &str, id: &str, data: &serde_json::Value) -> Result<()>;
    async fn get_runtime(&self, tenant: &str, id: &str) -> Result<Option<serde_json::Value>>;
    async fn list_runtimes(&self, tenant: &str) -> Result<Vec<serde_json::Value>>;
    async fn delete_runtime(&self, tenant: &str, id: &str) -> Result<bool>;

    async fn upsert_route(&self, tenant: &str, id: &str, data: &serde_json::Value) -> Result<()>;
    async fn get_route(&self, tenant: &str, id: &str) -> Result<Option<serde_json::Value>>;
    async fn list_routes(&self, tenant: &str) -> Result<Vec<serde_json::Value>>;
    async fn delete_route(&self, tenant: &str, id: &str) -> Result<bool>;
}

#[async_trait::async_trait]
pub trait ObservabilityStore: Send + Sync {
    async fn insert_observation_event(&self, event: &AgentObservationEvent) -> Result<()>;
    async fn list_observation_events(&self, tenant_id: &str) -> Result<Vec<AgentObservationEvent>>;
    async fn insert_cost_ledger(&self, entry: &CostLedgerEntry) -> Result<()>;
    async fn list_cost_ledger(&self) -> Result<Vec<CostLedgerEntry>>;
    async fn upsert_policy_suggestion(&self, suggestion: &PolicySuggestion) -> Result<()>;
    async fn list_policy_suggestions(&self, tenant_id: &str) -> Result<Vec<PolicySuggestion>>;
}

pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteStore {
    fn run_migrations(conn: &mut Connection) -> Result<()> {
        let migrations = [
            include_str!("../migrations/20260609000000_init.sql"),
            include_str!("../migrations/20260609000001_bundle_blobs.sql"),
            include_str!("../migrations/20260609000002_telemetry_events.sql"),
            include_str!("../migrations/20260620000000_observability_and_policy_suggestions.sql"),
            include_str!("../migrations/20260621000000_pdp_runtimes_and_routes.sql"),
            include_str!("../migrations/20260622000000_policy_preset_deployments.sql"),
            include_str!("../migrations/20260622000001_agent_inventory.sql"),
        ];

        let tx = conn.transaction()?;
        tx.execute(
            "CREATE TABLE IF NOT EXISTS _migrations (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
            [],
        )?;

        for (i, sql) in migrations.iter().enumerate() {
            let id = i as i64;
            let count: i64 = tx.query_row(
                "SELECT count(*) FROM _migrations WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )?;
            if count == 0 {
                tx.execute_batch(sql)?;
                tx.execute(
                    "INSERT INTO _migrations (id, name) VALUES (?1, ?2)",
                    params![id, format!("mig_{}", id)],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub async fn new(db_url: &str) -> Result<Self> {
        let db_path = db_url
            .strip_prefix("sqlite://")
            .unwrap_or(db_url)
            .split('?')
            .next()
            .unwrap_or("")
            .to_string();
        let conn = tokio::task::spawn_blocking(move || -> Result<Connection> {
            let mut conn =
                if db_path == ":memory:" || db_path == "sqlite::memory:" || db_path.is_empty() {
                    Connection::open_in_memory()?
                } else {
                    Connection::open(&db_path)?
                };

            Self::run_migrations(&mut conn)?;

            Ok(conn)
        })
        .await??;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    async fn upsert_object<T: Serialize>(
        &self,
        tenant_id: &str,
        object_type: &str,
        object_id: &str,
        status: &str,
        source: &str,
        data: &T,
    ) -> Result<()> {
        let json_data = serde_json::to_string(data)?;
        let now = chrono::Utc::now().to_rfc3339();

        let tenant_id = tenant_id.to_string();
        let object_type = object_type.to_string();
        let object_id = object_id.to_string();
        let status = status.to_string();
        let source = source.to_string();

        let conn_arc = self.conn.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let conn = conn_arc.lock().unwrap();
            conn.execute(
                r#"
                INSERT INTO registry_objects (tenant_id, object_type, object_id, status, source, data_json, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
                ON CONFLICT(tenant_id, object_type, object_id) DO UPDATE SET
                    status=excluded.status,
                    source=excluded.source,
                    data_json=excluded.data_json,
                    updated_at=excluded.updated_at
                "#,
                params![tenant_id, object_type, object_id, status, source, json_data, now],
            )?;
            Ok(())
        }).await??;

        Ok(())
    }

    async fn get_object<T: for<'de> Deserialize<'de>>(
        &self,
        tenant_id: &str,
        object_type: &str,
        object_id: &str,
    ) -> Result<Option<T>> {
        let tenant_id = tenant_id.to_string();
        let object_type = object_type.to_string();
        let object_id = object_id.to_string();

        let conn_arc = self.conn.clone();
        let data_json = tokio::task::spawn_blocking(move || -> Result<Option<String>> {
            let conn = conn_arc.lock().unwrap();
            let mut stmt = conn.prepare("SELECT data_json FROM registry_objects WHERE tenant_id = ?1 AND object_type = ?2 AND object_id = ?3")?;
            let mut rows = stmt.query(params![tenant_id, object_type, object_id])?;
            if let Some(row) = rows.next()? {
                Ok(Some(row.get(0)?))
            } else {
                Ok(None)
            }
        }).await??;

        if let Some(json) = data_json {
            let obj: T = serde_json::from_str(&json)?;
            Ok(Some(obj))
        } else {
            Ok(None)
        }
    }

    async fn list_objects<T: for<'de> Deserialize<'de>>(
        &self,
        tenant_id: &str,
        object_type: &str,
    ) -> Result<Vec<T>> {
        let tenant_id = tenant_id.to_string();
        let object_type = object_type.to_string();

        let conn_arc = self.conn.clone();
        let data_jsons = tokio::task::spawn_blocking(move || -> Result<Vec<String>> {
            let conn = conn_arc.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT data_json FROM registry_objects WHERE tenant_id = ?1 AND object_type = ?2",
            )?;
            let mut rows = stmt.query(params![tenant_id, object_type])?;
            let mut results = Vec::new();
            while let Some(row) = rows.next()? {
                results.push(row.get(0)?);
            }
            Ok(results)
        })
        .await??;

        let mut results = Vec::new();
        for json in data_jsons {
            let obj: T = serde_json::from_str(&json)?;
            results.push(obj);
        }
        Ok(results)
    }

    async fn delete_object(
        &self,
        tenant_id: &str,
        object_type: &str,
        object_id: &str,
    ) -> Result<bool> {
        let tenant_id = tenant_id.to_string();
        let object_type = object_type.to_string();
        let object_id = object_id.to_string();

        let conn_arc = self.conn.clone();
        let rows_affected = tokio::task::spawn_blocking(move || -> Result<usize> {
            let conn = conn_arc.lock().unwrap();
            let changed = conn.execute(
                "DELETE FROM registry_objects WHERE tenant_id = ?1 AND object_type = ?2 AND object_id = ?3",
                params![tenant_id, object_type, object_id],
            )?;
            Ok(changed)
        }).await??;

        Ok(rows_affected > 0)
    }
}

#[async_trait::async_trait]
impl RegistryStore for SqliteStore {
    async fn upsert_agent(&self, agent: AiAgent) -> Result<AiAgent> {
        let status = serde_json::to_string(&agent.meta.status)?.replace("\"", "");
        let source = serde_json::to_string(&agent.meta.source)?.replace("\"", "");
        self.upsert_object(
            &agent.meta.tenant_id,
            "agent",
            &agent.agent_id,
            &status,
            &source,
            &agent,
        )
        .await?;
        Ok(agent)
    }

    async fn get_agent(&self, tenant_id: &str, agent_id: &str) -> Result<Option<AiAgent>> {
        self.get_object(tenant_id, "agent", agent_id).await
    }

    async fn upsert_raw(
        &self,
        tenant_id: &str,
        object_type: &str,
        object_id: &str,
        data: &serde_json::Value,
    ) -> Result<()> {
        let json_data = serde_json::to_string(data)?;
        let now = chrono::Utc::now().to_rfc3339();

        let tenant_id = tenant_id.to_string();
        let object_type = object_type.to_string();
        let object_id = object_id.to_string();

        let conn_arc = self.conn.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let conn = conn_arc.lock().unwrap();
            conn.execute(
                r#"
                INSERT INTO registry_objects (tenant_id, object_type, object_id, status, source, data_json, created_at, updated_at)
                VALUES (?1, ?2, ?3, 'raw', 'raw', ?4, ?5, ?5)
                ON CONFLICT(tenant_id, object_type, object_id) DO UPDATE SET
                    data_json=excluded.data_json,
                    updated_at=excluded.updated_at
                "#,
                params![tenant_id, object_type, object_id, json_data, now],
            )?;
            Ok(())
        })
        .await??;
        Ok(())
    }

    async fn get_raw(
        &self,
        tenant_id: &str,
        object_type: &str,
        object_id: &str,
    ) -> Result<Option<serde_json::Value>> {
        let tenant_id = tenant_id.to_string();
        let object_type = object_type.to_string();
        let object_id = object_id.to_string();

        let conn_arc = self.conn.clone();
        let json_str = tokio::task::spawn_blocking(move || -> Result<Option<String>> {
            let conn = conn_arc.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT data_json FROM registry_objects WHERE tenant_id = ?1 AND object_type = ?2 AND object_id = ?3"
            )?;
            let mut rows = stmt.query(params![tenant_id, object_type, object_id])?;
            if let Some(row) = rows.next()? {
                Ok(Some(row.get(0)?))
            } else {
                Ok(None)
            }
        })
        .await??;

        if let Some(s) = json_str {
            let data: serde_json::Value = serde_json::from_str(&s)?;
            Ok(Some(data))
        } else {
            Ok(None)
        }
    }

    async fn list_raw(&self, tenant_id: &str, object_type: &str) -> Result<Vec<serde_json::Value>> {
        let tenant_id = tenant_id.to_string();
        let object_type = object_type.to_string();

        let conn_arc = self.conn.clone();
        let json_strs = tokio::task::spawn_blocking(move || -> Result<Vec<String>> {
            let conn = conn_arc.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT data_json FROM registry_objects WHERE tenant_id = ?1 AND object_type = ?2",
            )?;
            let mut rows = stmt.query(params![tenant_id, object_type])?;
            let mut out = Vec::new();
            while let Some(row) = rows.next()? {
                out.push(row.get(0)?);
            }
            Ok(out)
        })
        .await??;

        let mut out = Vec::new();
        for s in json_strs {
            if let Ok(data) = serde_json::from_str(&s) {
                out.push(data);
            }
        }
        Ok(out)
    }

    async fn list_agents(&self, tenant_id: &str) -> Result<Vec<AiAgent>> {
        self.list_objects(tenant_id, "agent").await
    }

    async fn delete_agent(&self, tenant_id: &str, agent_id: &str) -> Result<bool> {
        self.delete_object(tenant_id, "agent", agent_id).await
    }

    async fn upsert_blackbox_ai(&self, provider: BlackboxAiProvider) -> Result<BlackboxAiProvider> {
        let status = serde_json::to_string(&provider.meta.status)?.replace("\"", "");
        let source = serde_json::to_string(&provider.meta.source)?.replace("\"", "");
        self.upsert_object(
            &provider.meta.tenant_id,
            "blackbox_ai",
            &provider.provider_id,
            &status,
            &source,
            &provider,
        )
        .await?;
        Ok(provider)
    }

    async fn get_blackbox_ai(
        &self,
        tenant_id: &str,
        provider_id: &str,
    ) -> Result<Option<BlackboxAiProvider>> {
        self.get_object(tenant_id, "blackbox_ai", provider_id).await
    }

    async fn list_blackbox_ai(&self, tenant_id: &str) -> Result<Vec<BlackboxAiProvider>> {
        self.list_objects(tenant_id, "blackbox_ai").await
    }

    async fn delete_blackbox_ai(&self, tenant_id: &str, provider_id: &str) -> Result<bool> {
        self.delete_object(tenant_id, "blackbox_ai", provider_id)
            .await
    }

    async fn upsert_entity(&self, entity: Entity) -> Result<Entity> {
        let status = serde_json::to_string(&entity.meta.status)?.replace("\"", "");
        let source = serde_json::to_string(&entity.meta.source)?.replace("\"", "");
        self.upsert_object(
            &entity.meta.tenant_id,
            "entity",
            &entity.entity_id,
            &status,
            &source,
            &entity,
        )
        .await?;
        Ok(entity)
    }

    async fn get_entity(&self, tenant_id: &str, entity_id: &str) -> Result<Option<Entity>> {
        self.get_object(tenant_id, "entity", entity_id).await
    }

    async fn list_entities(&self, tenant_id: &str) -> Result<Vec<Entity>> {
        self.list_objects(tenant_id, "entity").await
    }

    async fn delete_entity(&self, tenant_id: &str, entity_id: &str) -> Result<bool> {
        self.delete_object(tenant_id, "entity", entity_id).await
    }

    async fn upsert_resource(&self, resource: Resource) -> Result<Resource> {
        let status = serde_json::to_string(&resource.meta.status)?.replace("\"", "");
        let source = serde_json::to_string(&resource.meta.source)?.replace("\"", "");
        self.upsert_object(
            &resource.meta.tenant_id,
            "resource",
            &resource.resource_id,
            &status,
            &source,
            &resource,
        )
        .await?;
        Ok(resource)
    }

    async fn get_resource(&self, tenant_id: &str, resource_id: &str) -> Result<Option<Resource>> {
        self.get_object(tenant_id, "resource", resource_id).await
    }

    async fn list_resources(&self, tenant_id: &str) -> Result<Vec<Resource>> {
        self.list_objects(tenant_id, "resource").await
    }

    async fn delete_resource(&self, tenant_id: &str, resource_id: &str) -> Result<bool> {
        self.delete_object(tenant_id, "resource", resource_id).await
    }

    async fn upsert_tool(&self, tool: Tool) -> Result<Tool> {
        let status = serde_json::to_string(&tool.meta.status)?.replace("\"", "");
        let source = serde_json::to_string(&tool.meta.source)?.replace("\"", "");
        self.upsert_object(
            &tool.meta.tenant_id,
            "tool",
            &tool.tool_id,
            &status,
            &source,
            &tool,
        )
        .await?;
        Ok(tool)
    }

    async fn get_tool(&self, tenant_id: &str, tool_id: &str) -> Result<Option<Tool>> {
        self.get_object(tenant_id, "tool", tool_id).await
    }

    async fn list_tools(&self, tenant_id: &str) -> Result<Vec<Tool>> {
        self.list_objects(tenant_id, "tool").await
    }

    async fn delete_tool(&self, tenant_id: &str, tool_id: &str) -> Result<bool> {
        self.delete_object(tenant_id, "tool", tool_id).await
    }

    async fn upsert_mcp_server(&self, server: McpServer) -> Result<McpServer> {
        let status = serde_json::to_string(&server.meta.status)?.replace("\"", "");
        let source = serde_json::to_string(&server.meta.source)?.replace("\"", "");
        self.upsert_object(
            &server.meta.tenant_id,
            "mcp_server",
            &server.server_id,
            &status,
            &source,
            &server,
        )
        .await?;
        Ok(server)
    }

    async fn get_mcp_server(&self, tenant_id: &str, server_id: &str) -> Result<Option<McpServer>> {
        self.get_object(tenant_id, "mcp_server", server_id).await
    }

    async fn list_mcp_servers(&self, tenant_id: &str) -> Result<Vec<McpServer>> {
        self.list_objects(tenant_id, "mcp_server").await
    }

    async fn delete_mcp_server(&self, tenant_id: &str, server_id: &str) -> Result<bool> {
        self.delete_object(tenant_id, "mcp_server", server_id).await
    }

    async fn upsert_relationship(&self, relationship: Relationship) -> Result<Relationship> {
        let status = serde_json::to_string(&relationship.meta.status)?.replace("\"", "");
        let source = serde_json::to_string(&relationship.meta.source)?.replace("\"", "");
        self.upsert_object(
            &relationship.meta.tenant_id,
            "relationship",
            &relationship.relationship_id,
            &status,
            &source,
            &relationship,
        )
        .await?;
        Ok(relationship)
    }

    async fn get_relationship(
        &self,
        tenant_id: &str,
        relationship_id: &str,
    ) -> Result<Option<Relationship>> {
        self.get_object(tenant_id, "relationship", relationship_id)
            .await
    }

    async fn list_relationships(&self, tenant_id: &str) -> Result<Vec<Relationship>> {
        self.list_objects(tenant_id, "relationship").await
    }

    async fn delete_relationship(&self, tenant_id: &str, relationship_id: &str) -> Result<bool> {
        self.delete_object(tenant_id, "relationship", relationship_id)
            .await
    }

    async fn upsert_agent_inventory(
        &self,
        inventory: dek_domain_schema::AgentCapabilityInventory,
    ) -> Result<dek_domain_schema::AgentCapabilityInventory> {
        let json_data = serde_json::to_string(&inventory)?;
        let now = chrono::Utc::now().timestamp();
        let conn_arc = self.conn.clone();
        let tenant_id = inventory.tenant_id.clone();
        let agent_id = inventory.agent_id.clone();
        let device_id = inventory.device_id.clone();

        tokio::task::spawn_blocking(move || -> Result<()> {
            let conn = conn_arc.lock().unwrap();
            conn.execute(
                r#"
                INSERT INTO agent_inventory (tenant, agent_id, device_id, inventory_json, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5)
                ON CONFLICT(tenant, agent_id) DO UPDATE SET
                    device_id=excluded.device_id,
                    inventory_json=excluded.inventory_json,
                    updated_at=excluded.updated_at
                "#,
                params![tenant_id, agent_id, device_id, json_data, now],
            )?;
            Ok(())
        })
        .await??;
        Ok(inventory)
    }

    async fn get_agent_inventory(
        &self,
        tenant_id: &str,
        agent_id: &str,
    ) -> Result<Option<dek_domain_schema::AgentCapabilityInventory>> {
        let tenant_id = tenant_id.to_string();
        let agent_id = agent_id.to_string();

        let conn_arc = self.conn.clone();
        let json_str = tokio::task::spawn_blocking(move || -> Result<Option<String>> {
            let conn = conn_arc.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT inventory_json FROM agent_inventory WHERE tenant = ?1 AND agent_id = ?2",
            )?;
            let mut rows = stmt.query(params![tenant_id, agent_id])?;
            if let Some(row) = rows.next()? {
                Ok(Some(row.get(0)?))
            } else {
                Ok(None)
            }
        })
        .await??;

        if let Some(json_str) = json_str {
            let inv: dek_domain_schema::AgentCapabilityInventory = serde_json::from_str(&json_str)?;
            Ok(Some(inv))
        } else {
            Ok(None)
        }
    }

    async fn list_agent_inventories(
        &self,
        tenant_id: &str,
    ) -> Result<Vec<dek_domain_schema::AgentCapabilityInventory>> {
        let tenant_id = tenant_id.to_string();
        let conn_arc = self.conn.clone();
        let json_strs = tokio::task::spawn_blocking(move || -> Result<Vec<String>> {
            let conn = conn_arc.lock().unwrap();
            let mut stmt =
                conn.prepare("SELECT inventory_json FROM agent_inventory WHERE tenant = ?1")?;
            let mut rows = stmt.query(params![tenant_id])?;
            let mut out = Vec::new();
            while let Some(row) = rows.next()? {
                out.push(row.get(0)?);
            }
            Ok(out)
        })
        .await??;

        let mut out = Vec::new();
        for json_str in json_strs {
            if let Ok(inv) = serde_json::from_str(&json_str) {
                out.push(inv);
            }
        }
        Ok(out)
    }

    async fn delete_agent_inventory(&self, tenant_id: &str, agent_id: &str) -> Result<bool> {
        let tenant_id = tenant_id.to_string();
        let agent_id = agent_id.to_string();
        let conn_arc = self.conn.clone();
        let rows_affected = tokio::task::spawn_blocking(move || -> Result<usize> {
            let conn = conn_arc.lock().unwrap();
            let changed = conn.execute(
                "DELETE FROM agent_inventory WHERE tenant = ?1 AND agent_id = ?2",
                params![tenant_id, agent_id],
            )?;
            Ok(changed)
        })
        .await??;
        Ok(rows_affected > 0)
    }
}

#[async_trait::async_trait]
impl PolicyStore for SqliteStore {
    async fn upsert_policy(
        &self,
        policy: dek_control_plane_api::policy::PolicyDraft,
    ) -> Result<dek_control_plane_api::policy::PolicyDraft> {
        let status = serde_json::to_string(&policy.meta.status)?.replace("\"", "");
        let source = serde_json::to_string(&policy.meta.source)?.replace("\"", "");
        self.upsert_object(
            &policy.meta.tenant_id,
            "policy_draft",
            &policy.policy_id,
            &status,
            &source,
            &policy,
        )
        .await?;
        Ok(policy)
    }

    async fn get_policy(
        &self,
        tenant_id: &str,
        policy_id: &str,
    ) -> Result<Option<dek_control_plane_api::policy::PolicyDraft>> {
        self.get_object(tenant_id, "policy_draft", policy_id).await
    }

    async fn list_policies(
        &self,
        tenant_id: &str,
    ) -> Result<Vec<dek_control_plane_api::policy::PolicyDraft>> {
        self.list_objects(tenant_id, "policy_draft").await
    }

    async fn delete_policy(&self, tenant_id: &str, policy_id: &str) -> Result<bool> {
        self.delete_object(tenant_id, "policy_draft", policy_id)
            .await
    }

    async fn put_policy_status(
        &self,
        tenant_id: &str,
        policy_id: &str,
        status: dek_control_plane_api::policy::PolicyLifecycleStatus,
    ) -> Result<()> {
        let mut policy = match self.get_policy(tenant_id, policy_id).await? {
            Some(p) => p,
            None => anyhow::bail!("Policy not found"),
        };
        // Just map it back to RegistryStatus string for meta status
        policy.meta.status = serde_json::from_value(serde_json::to_value(status)?)?;
        self.upsert_policy(policy).await?;
        Ok(())
    }

    async fn upsert_policy_raw(
        &self,
        tenant: &str,
        id: &str,
        data: &serde_json::Value,
    ) -> Result<()> {
        self.upsert_object(tenant, "policy_raw", id, "active", "local", data)
            .await
    }

    async fn get_policy_raw(&self, tenant: &str, id: &str) -> Result<Option<serde_json::Value>> {
        self.get_object(tenant, "policy_raw", id).await
    }

    async fn put_blob(&self, tenant: &str, path: &str, bytes: &[u8]) -> Result<()> {
        let tenant = tenant.to_string();
        let path = path.to_string();
        let bytes = bytes.to_vec();
        let conn_arc = self.conn.clone();

        tokio::task::spawn_blocking(move || -> Result<()> {
            let conn = conn_arc.lock().unwrap();
            conn.execute(
                "INSERT INTO bundle_blobs (tenant_id, path, bytes) VALUES (?1, ?2, ?3) ON CONFLICT(tenant_id, path) DO UPDATE SET bytes=excluded.bytes",
                params![tenant, path, bytes]
            )?;
            Ok(())
        }).await??;
        Ok(())
    }

    async fn get_blob(&self, tenant: &str, path: &str) -> Result<Option<Vec<u8>>> {
        let tenant = tenant.to_string();
        let path = path.to_string();
        let conn_arc = self.conn.clone();

        let bytes = tokio::task::spawn_blocking(move || -> Result<Option<Vec<u8>>> {
            let conn = conn_arc.lock().unwrap();
            let mut stmt =
                conn.prepare("SELECT bytes FROM bundle_blobs WHERE tenant_id = ?1 AND path = ?2")?;
            let mut rows = stmt.query(params![tenant, path])?;
            if let Some(row) = rows.next()? {
                Ok(Some(row.get(0)?))
            } else {
                Ok(None)
            }
        })
        .await??;

        Ok(bytes)
    }
    async fn upsert_preset_deployment(
        &self,
        tenant_id: &str,
        deployment_id: &str,
        data: &serde_json::Value,
    ) -> Result<()> {
        let preset_id = data.get("preset_id").and_then(|v| v.as_str()).unwrap_or("");
        let preset_version = data
            .get("preset_version")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let control_mode = data
            .get("control_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("Observe");
        let status = data
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("active");
        let target_scopes_json = data
            .get("targets")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "{}".to_string());
        let parameters_json = data
            .get("params")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "{}".to_string());
        let tenant_id = tenant_id.to_string();
        let deployment_id = deployment_id.to_string();
        let preset_id = preset_id.to_string();
        let preset_version = preset_version.to_string();
        let control_mode = control_mode.to_string();
        let status = status.to_string();
        let now = chrono::Utc::now().to_rfc3339();

        let conn_arc = self.conn.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let conn = conn_arc.lock().unwrap();
            conn.execute(
                r#"
                INSERT INTO policy_preset_deployments (
                    tenant_id, deployment_id, preset_id, preset_version, control_mode, status, target_scopes_json, parameters_json, created_at, updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)
                ON CONFLICT(tenant_id, deployment_id) DO UPDATE SET
                    status=excluded.status,
                    control_mode=excluded.control_mode,
                    target_scopes_json=excluded.target_scopes_json,
                    parameters_json=excluded.parameters_json,
                    updated_at=excluded.updated_at
                "#,
                params![tenant_id, deployment_id, preset_id, preset_version, control_mode, status, target_scopes_json, parameters_json, now]
            )?;
            Ok(())
        }).await??;
        Ok(())
    }

    async fn get_preset_deployment(
        &self,
        tenant_id: &str,
        deployment_id: &str,
    ) -> Result<Option<serde_json::Value>> {
        let tenant_id = tenant_id.to_string();
        let deployment_id = deployment_id.to_string();
        let conn_arc = self.conn.clone();

        let val = tokio::task::spawn_blocking(move || -> Result<Option<serde_json::Value>> {
            let conn = conn_arc.lock().unwrap();
            let mut stmt = conn.prepare("SELECT * FROM policy_preset_deployments WHERE tenant_id = ?1 AND deployment_id = ?2")?;
            let mut rows = stmt.query(params![tenant_id, deployment_id])?;
            if let Some(r) = rows.next()? {
                let mut val = serde_json::json!({
                    "deployment_id": r.get::<_, String>("deployment_id")?,
                    "preset_id": r.get::<_, String>("preset_id")?,
                    "preset_version": r.get::<_, Option<String>>("preset_version")?.unwrap_or_default(),
                    "control_mode": r.get::<_, String>("control_mode")?,
                    "status": r.get::<_, String>("status")?,
                    "created_at": r.get::<_, String>("created_at")?,
                    "updated_at": r.get::<_, String>("updated_at")?,
                });
                let tj: String = r.get("target_scopes_json")?;
                let pj: String = r.get("parameters_json")?;
                val["targets"] = serde_json::from_str(&tj).unwrap_or(serde_json::json!({}));
                val["params"] = serde_json::from_str(&pj).unwrap_or(serde_json::json!({}));
                Ok(Some(val))
            } else {
                Ok(None)
            }
        }).await??;

        Ok(val)
    }

    async fn list_preset_deployments(&self, tenant_id: &str) -> Result<Vec<serde_json::Value>> {
        let tenant_id = tenant_id.to_string();
        let conn_arc = self.conn.clone();

        let out = tokio::task::spawn_blocking(move || -> Result<Vec<serde_json::Value>> {
            let conn = conn_arc.lock().unwrap();
            let mut stmt = conn.prepare("SELECT * FROM policy_preset_deployments WHERE tenant_id = ?1")?;
            let mut rows = stmt.query(params![tenant_id])?;
            let mut out = Vec::new();
            while let Some(r) = rows.next()? {
                let mut val = serde_json::json!({
                    "deployment_id": r.get::<_, String>("deployment_id")?,
                    "preset_id": r.get::<_, String>("preset_id")?,
                    "preset_version": r.get::<_, Option<String>>("preset_version")?.unwrap_or_default(),
                    "control_mode": r.get::<_, String>("control_mode")?,
                    "status": r.get::<_, String>("status")?,
                    "created_at": r.get::<_, String>("created_at")?,
                    "updated_at": r.get::<_, String>("updated_at")?,
                });
                let tj: String = r.get("target_scopes_json")?;
                let pj: String = r.get("parameters_json")?;
                val["targets"] = serde_json::from_str(&tj).unwrap_or(serde_json::json!({}));
                val["params"] = serde_json::from_str(&pj).unwrap_or(serde_json::json!({}));
                out.push(val);
            }
            Ok(out)
        }).await??;

        Ok(out)
    }

    async fn upsert_pep_binding(
        &self,
        tenant_id: &str,
        binding_id: &str,
        deployment_id: &str,
        pep_type: &str,
        data: &serde_json::Value,
    ) -> Result<()> {
        let config_json = serde_json::to_string(data)?;
        let status = data
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("active");
        let tenant_id = tenant_id.to_string();
        let binding_id = binding_id.to_string();
        let deployment_id = deployment_id.to_string();
        let pep_type = pep_type.to_string();
        let status = status.to_string();
        let now = chrono::Utc::now().to_rfc3339();

        let conn_arc = self.conn.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let conn = conn_arc.lock().unwrap();
            conn.execute(
                r#"
                INSERT INTO pep_bindings (
                    tenant_id, binding_id, deployment_id, pep_type, config_json, status, created_at, updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
                ON CONFLICT(tenant_id, binding_id) DO UPDATE SET
                    config_json=excluded.config_json,
                    status=excluded.status,
                    updated_at=excluded.updated_at
                "#,
                params![tenant_id, binding_id, deployment_id, pep_type, config_json, status, now]
            )?;
            Ok(())
        }).await??;
        Ok(())
    }

    async fn list_pep_bindings(
        &self,
        tenant_id: &str,
        deployment_id: &str,
    ) -> Result<Vec<serde_json::Value>> {
        let tenant_id = tenant_id.to_string();
        let deployment_id = deployment_id.to_string();
        let conn_arc = self.conn.clone();

        let out = tokio::task::spawn_blocking(move || -> Result<Vec<serde_json::Value>> {
            let conn = conn_arc.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT * FROM pep_bindings WHERE tenant_id = ?1 AND deployment_id = ?2",
            )?;
            let mut rows = stmt.query(params![tenant_id, deployment_id])?;
            let mut out = Vec::new();
            while let Some(r) = rows.next()? {
                let mut val = serde_json::json!({
                    "binding_id": r.get::<_, String>("binding_id")?,
                    "deployment_id": r.get::<_, String>("deployment_id")?,
                    "pep_type": r.get::<_, String>("pep_type")?,
                    "status": r.get::<_, String>("status")?,
                    "created_at": r.get::<_, String>("created_at")?,
                    "updated_at": r.get::<_, String>("updated_at")?,
                });
                let cj: String = r.get("config_json")?;
                val["config"] = serde_json::from_str(&cj).unwrap_or(serde_json::json!({}));
                out.push(val);
            }
            Ok(out)
        })
        .await??;

        Ok(out)
    }
}

#[async_trait::async_trait]
impl TelemetryStore for SqliteStore {
    async fn put_telemetry(
        &self,
        tenant: &str,
        kind: &str,
        event_id: &str,
        data: &serde_json::Value,
    ) -> Result<()> {
        let tenant = tenant.to_string();
        let kind = kind.to_string();
        let event_id = event_id.to_string();
        let data_json = serde_json::to_string(data)?;
        let now = chrono::Utc::now().to_rfc3339();

        let conn_arc = self.conn.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let conn = conn_arc.lock().unwrap();
            conn.execute(
                "INSERT INTO telemetry_events (tenant_id, event_type, event_id, data_json, created_at)
                 VALUES (?1,?2,?3,?4,?5)
                 ON CONFLICT(tenant_id,event_id) DO UPDATE SET data_json=?4",
                 params![tenant, kind, event_id, data_json, now]
            )?;
            Ok(())
        }).await??;
        Ok(())
    }
    async fn list_telemetry(&self, tenant: &str, kind: &str) -> Result<Vec<serde_json::Value>> {
        let tenant = tenant.to_string();
        let kind = kind.to_string();
        let conn_arc = self.conn.clone();

        let rows = tokio::task::spawn_blocking(move || -> Result<Vec<String>> {
            let conn = conn_arc.lock().unwrap();
            let mut stmt = conn.prepare("SELECT data_json FROM telemetry_events WHERE tenant_id=?1 AND event_type=?2 ORDER BY created_at DESC LIMIT 1000")?;
            let mut rows = stmt.query(params![tenant, kind])?;
            let mut out = Vec::new();
            while let Some(row) = rows.next()? {
                out.push(row.get(0)?);
            }
            Ok(out)
        }).await??;

        Ok(rows
            .into_iter()
            .filter_map(|j| serde_json::from_str(&j).ok())
            .collect())
    }
}

#[async_trait::async_trait]
impl PdpStore for SqliteStore {
    async fn upsert_runtime(&self, tenant: &str, id: &str, data: &serde_json::Value) -> Result<()> {
        let name = data.get("name").and_then(|v| v.as_str()).unwrap_or(id);
        let category = data
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("external_connector");
        let kind = data
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("custom_http");
        let enabled = data
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let status = data
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("ready");
        let endpoint = data.get("endpoint").and_then(|v| v.as_str());
        let auth_ref = data.get("auth_ref").and_then(|v| v.as_str());
        let capabilities_json = data
            .get("capabilities")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "[]".to_string());
        let health_json = data.get("health").map(|v| v.to_string());

        let now = chrono::Utc::now().to_rfc3339();

        let tenant = tenant.to_string();
        let id = id.to_string();
        let name = name.to_string();
        let category = category.to_string();
        let kind = kind.to_string();
        let status = status.to_string();
        let endpoint = endpoint.map(|s| s.to_string());
        let auth_ref = auth_ref.map(|s| s.to_string());

        let conn_arc = self.conn.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let conn = conn_arc.lock().unwrap();
            conn.execute(
                r#"
                INSERT INTO pdp_runtimes (
                    tenant_id, id, name, category, kind, enabled, status, endpoint, auth_ref, capabilities_json, health_json, created_at, updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)
                ON CONFLICT(tenant_id, id) DO UPDATE SET
                    name=excluded.name,
                    category=excluded.category,
                    kind=excluded.kind,
                    enabled=excluded.enabled,
                    status=excluded.status,
                    endpoint=excluded.endpoint,
                    auth_ref=excluded.auth_ref,
                    capabilities_json=excluded.capabilities_json,
                    health_json=excluded.health_json,
                    updated_at=excluded.updated_at
                "#,
                params![tenant, id, name, category, kind, enabled, status, endpoint, auth_ref, capabilities_json, health_json, now]
            )?;
            Ok(())
        }).await??;

        Ok(())
    }

    async fn get_runtime(&self, tenant: &str, id: &str) -> Result<Option<serde_json::Value>> {
        let tenant = tenant.to_string();
        let id = id.to_string();
        let conn_arc = self.conn.clone();

        let out = tokio::task::spawn_blocking(move || -> Result<Option<serde_json::Value>> {
            let conn = conn_arc.lock().unwrap();
            let mut stmt =
                conn.prepare("SELECT * FROM pdp_runtimes WHERE tenant_id = ?1 AND id = ?2")?;
            let mut rows = stmt.query(params![tenant, id])?;
            if let Some(r) = rows.next()? {
                Ok(Some(Self::row_to_pdp_runtime(&r)?))
            } else {
                Ok(None)
            }
        })
        .await??;

        Ok(out)
    }

    async fn list_runtimes(&self, tenant: &str) -> Result<Vec<serde_json::Value>> {
        let tenant = tenant.to_string();
        let conn_arc = self.conn.clone();

        let out = tokio::task::spawn_blocking(move || -> Result<Vec<serde_json::Value>> {
            let conn = conn_arc.lock().unwrap();
            let mut stmt = conn.prepare("SELECT * FROM pdp_runtimes WHERE tenant_id = ?1")?;
            let mut rows = stmt.query(params![tenant])?;
            let mut results = Vec::new();
            while let Some(r) = rows.next()? {
                if let Ok(val) = Self::row_to_pdp_runtime(&r) {
                    results.push(val);
                }
            }
            Ok(results)
        })
        .await??;

        Ok(out)
    }

    async fn delete_runtime(&self, tenant: &str, id: &str) -> Result<bool> {
        let tenant = tenant.to_string();
        let id = id.to_string();
        let conn_arc = self.conn.clone();

        let rows_affected = tokio::task::spawn_blocking(move || -> Result<usize> {
            let conn = conn_arc.lock().unwrap();
            Ok(conn.execute(
                "DELETE FROM pdp_runtimes WHERE tenant_id = ?1 AND id = ?2",
                params![tenant, id],
            )?)
        })
        .await??;

        Ok(rows_affected > 0)
    }

    async fn upsert_route(&self, tenant: &str, id: &str, data: &serde_json::Value) -> Result<()> {
        let name = data.get("name").and_then(|v| v.as_str()).unwrap_or(id);
        let enabled = data
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let priority = data.get("priority").and_then(|v| v.as_i64()).unwrap_or(0);
        let match_cond_json = data
            .get("match")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "{}".to_string());
        let mode = data
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("local_only");
        let primary_pdp_id = data
            .get("primary_pdp_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let fallback_pdp_ids_json = data
            .get("fallback_pdp_ids")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "[]".to_string());
        let shadow_pdp_ids_json = data
            .get("shadow_pdp_ids")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "[]".to_string());
        let merge_strategy = data
            .get("merge_strategy")
            .and_then(|v| v.as_str())
            .unwrap_or("override");
        let failure_behavior = data
            .get("failure_behavior")
            .and_then(|v| v.as_str())
            .unwrap_or("deny");
        let timeout_ms = data
            .get("timeout_ms")
            .and_then(|v| v.as_i64())
            .unwrap_or(1000);
        let max_retries = data
            .get("max_retries")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        let now = chrono::Utc::now().to_rfc3339();

        let tenant = tenant.to_string();
        let id = id.to_string();
        let name = name.to_string();
        let mode = mode.to_string();
        let primary_pdp_id = primary_pdp_id.to_string();
        let merge_strategy = merge_strategy.to_string();
        let failure_behavior = failure_behavior.to_string();

        let conn_arc = self.conn.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let conn = conn_arc.lock().unwrap();
            conn.execute(
                r#"
                INSERT INTO pdp_routes (
                    tenant_id, id, name, enabled, priority, match_cond_json, mode, primary_pdp_id, fallback_pdp_ids_json, shadow_pdp_ids_json, merge_strategy, failure_behavior, timeout_ms, max_retries, created_at, updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15)
                ON CONFLICT(tenant_id, id) DO UPDATE SET
                    name=excluded.name,
                    enabled=excluded.enabled,
                    priority=excluded.priority,
                    match_cond_json=excluded.match_cond_json,
                    mode=excluded.mode,
                    primary_pdp_id=excluded.primary_pdp_id,
                    fallback_pdp_ids_json=excluded.fallback_pdp_ids_json,
                    shadow_pdp_ids_json=excluded.shadow_pdp_ids_json,
                    merge_strategy=excluded.merge_strategy,
                    failure_behavior=excluded.failure_behavior,
                    timeout_ms=excluded.timeout_ms,
                    max_retries=excluded.max_retries,
                    updated_at=excluded.updated_at
                "#,
                params![tenant, id, name, enabled, priority, match_cond_json, mode, primary_pdp_id, fallback_pdp_ids_json, shadow_pdp_ids_json, merge_strategy, failure_behavior, timeout_ms, max_retries, now]
            )?;
            Ok(())
        }).await??;

        Ok(())
    }

    async fn get_route(&self, tenant: &str, id: &str) -> Result<Option<serde_json::Value>> {
        let tenant = tenant.to_string();
        let id = id.to_string();
        let conn_arc = self.conn.clone();

        let out = tokio::task::spawn_blocking(move || -> Result<Option<serde_json::Value>> {
            let conn = conn_arc.lock().unwrap();
            let mut stmt =
                conn.prepare("SELECT * FROM pdp_routes WHERE tenant_id = ?1 AND id = ?2")?;
            let mut rows = stmt.query(params![tenant, id])?;
            if let Some(r) = rows.next()? {
                Ok(Some(Self::row_to_pdp_route(&r)?))
            } else {
                Ok(None)
            }
        })
        .await??;

        Ok(out)
    }

    async fn list_routes(&self, tenant: &str) -> Result<Vec<serde_json::Value>> {
        let tenant = tenant.to_string();
        let conn_arc = self.conn.clone();

        let out = tokio::task::spawn_blocking(move || -> Result<Vec<serde_json::Value>> {
            let conn = conn_arc.lock().unwrap();
            let mut stmt = conn
                .prepare("SELECT * FROM pdp_routes WHERE tenant_id = ?1 ORDER BY priority DESC")?;
            let mut rows = stmt.query(params![tenant])?;
            let mut results = Vec::new();
            while let Some(r) = rows.next()? {
                if let Ok(val) = Self::row_to_pdp_route(&r) {
                    results.push(val);
                }
            }
            Ok(results)
        })
        .await??;

        Ok(out)
    }

    async fn delete_route(&self, tenant: &str, id: &str) -> Result<bool> {
        let tenant = tenant.to_string();
        let id = id.to_string();
        let conn_arc = self.conn.clone();

        let rows_affected = tokio::task::spawn_blocking(move || -> Result<usize> {
            let conn = conn_arc.lock().unwrap();
            Ok(conn.execute(
                "DELETE FROM pdp_routes WHERE tenant_id = ?1 AND id = ?2",
                params![tenant, id],
            )?)
        })
        .await??;

        Ok(rows_affected > 0)
    }
}

impl SqliteStore {
    fn row_to_pdp_runtime(row: &rusqlite::Row<'_>) -> Result<serde_json::Value> {
        let id: String = row.get("id")?;
        let name: String = row.get("name")?;
        let category: String = row.get("category")?;
        let kind: String = row.get("kind")?;
        let enabled: bool = row.get("enabled")?;
        let status: String = row.get("status")?;
        let endpoint: Option<String> = row.get("endpoint")?;
        let auth_ref: Option<String> = row.get("auth_ref")?;
        let capabilities_json: String = row.get("capabilities_json")?;
        let health_json: Option<String> = row.get("health_json")?;
        let created_at: String = row.get("created_at")?;
        let updated_at: String = row.get("updated_at")?;

        let capabilities: serde_json::Value =
            serde_json::from_str(&capabilities_json).unwrap_or_else(|_| serde_json::json!([]));
        let health = health_json.and_then(|h| serde_json::from_str::<serde_json::Value>(&h).ok());

        let mut obj = serde_json::json!({
            "id": id,
            "name": name,
            "category": category,
            "kind": kind,
            "enabled": enabled,
            "status": status,
            "capabilities": capabilities,
            "created_at": created_at,
            "updated_at": updated_at
        });

        if let Some(ep) = endpoint {
            obj["endpoint"] = serde_json::Value::String(ep);
        }
        if let Some(ar) = auth_ref {
            obj["auth_ref"] = serde_json::Value::String(ar);
        }
        if let Some(h) = health {
            obj["health"] = h;
        }

        Ok(obj)
    }

    fn row_to_pdp_route(row: &rusqlite::Row<'_>) -> Result<serde_json::Value> {
        let id: String = row.get("id")?;
        let name: String = row.get("name")?;
        let enabled: bool = row.get("enabled")?;
        let priority: i64 = row.get("priority")?;
        let match_cond_json: String = row.get("match_cond_json")?;
        let mode: String = row.get("mode")?;
        let primary_pdp_id: String = row.get("primary_pdp_id")?;
        let fallback_pdp_ids_json: String = row.get("fallback_pdp_ids_json")?;
        let shadow_pdp_ids_json: String = row.get("shadow_pdp_ids_json")?;
        let merge_strategy: String = row.get("merge_strategy")?;
        let failure_behavior: String = row.get("failure_behavior")?;
        let timeout_ms: i64 = row.get("timeout_ms")?;
        let max_retries: i64 = row.get("max_retries")?;
        let created_at: String = row.get("created_at")?;
        let updated_at: String = row.get("updated_at")?;

        let match_cond: serde_json::Value =
            serde_json::from_str(&match_cond_json).unwrap_or_else(|_| serde_json::json!({}));
        let fallback_pdp_ids: serde_json::Value =
            serde_json::from_str(&fallback_pdp_ids_json).unwrap_or_else(|_| serde_json::json!([]));
        let shadow_pdp_ids: serde_json::Value =
            serde_json::from_str(&shadow_pdp_ids_json).unwrap_or_else(|_| serde_json::json!([]));

        Ok(serde_json::json!({
            "id": id,
            "name": name,
            "enabled": enabled,
            "priority": priority,
            "match": match_cond,
            "mode": mode,
            "primary_pdp_id": primary_pdp_id,
            "fallback_pdp_ids": fallback_pdp_ids,
            "shadow_pdp_ids": shadow_pdp_ids,
            "merge_strategy": merge_strategy,
            "failure_behavior": failure_behavior,
            "timeout_ms": timeout_ms,
            "max_retries": max_retries,
            "created_at": created_at,
            "updated_at": updated_at
        }))
    }
}

pub async fn seed_pdp_defaults(store: &Arc<dyn PdpStore>) -> Result<()> {
    let tenant = "local";

    use crate::pdp_models::*;
    let local_runtimes = vec![
        PdpRuntime {
            id: "opa_wasm".to_string(),
            name: "OPA WASM".to_string(),
            category: PdpRuntimeCategory::LocalEngine,
            kind: PdpKind::OpaWasm,
            mode: "passthrough".into(),
            system_managed: true,
            enabled: true,
            status: PdpStatus::Ready,
            endpoint: None,
            auth_ref: None,
            capabilities: vec![],
            config_source: "system".into(),
            active_bundle_id: None,
            active_bundle_hash: None,
            last_activated_at: None,
            last_probe: None,
            health: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        },
        PdpRuntime {
            id: "cedar_local".to_string(),
            name: "Cedar Local".to_string(),
            category: PdpRuntimeCategory::LocalEngine,
            kind: PdpKind::CedarLocal,
            mode: "passthrough".into(),
            system_managed: true,
            enabled: true,
            status: PdpStatus::Ready,
            endpoint: None,
            auth_ref: None,
            capabilities: vec![],
            config_source: "system".into(),
            active_bundle_id: None,
            active_bundle_hash: None,
            last_activated_at: None,
            last_probe: None,
            health: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        },
        PdpRuntime {
            id: "wasm_plugin".to_string(),
            name: "WASM Plugin".to_string(),
            category: PdpRuntimeCategory::LocalEngine,
            kind: PdpKind::WasmPlugin,
            mode: "passthrough".into(),
            system_managed: true,
            enabled: true,
            status: PdpStatus::Ready,
            endpoint: None,
            auth_ref: None,
            capabilities: vec![],
            config_source: "system".into(),
            active_bundle_id: None,
            active_bundle_hash: None,
            last_activated_at: None,
            last_probe: None,
            health: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        },
        PdpRuntime {
            id: "policy_router".to_string(),
            name: "Policy Router".to_string(),
            category: PdpRuntimeCategory::LocalEngine,
            kind: PdpKind::CustomHttp,
            mode: "passthrough".into(),
            system_managed: true,
            enabled: true,
            status: PdpStatus::Ready,
            endpoint: None,
            auth_ref: None,
            capabilities: vec![],
            config_source: "system".into(),
            active_bundle_id: None,
            active_bundle_hash: None,
            last_activated_at: None,
            last_probe: None,
            health: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        },
    ];

    for rt in local_runtimes {
        if store.get_runtime(tenant, &rt.id).await?.is_none() {
            let val = serde_json::to_value(&rt)?;
            store.upsert_runtime(tenant, &rt.id, &val).await?;
        }
    }

    let default_route_id = "default_route";
    if store.get_route(tenant, default_route_id).await?.is_none() {
        let route = PdpRouteRule {
            id: default_route_id.to_string(),
            name: "Default Route".to_string(),
            enabled: true,
            priority: 0,
            match_cond: RouteMatch {
                agent_ids: None,
                resource_ids: None,
                protocols: None,
                policy_tags: None,
                sensitivity: None,
                environment: None,
            },
            mode: PdpRouteMode::LocalPrimaryRemoteFallback,
            primary_pdp_id: "opa_wasm".to_string(),
            fallback_pdp_ids: vec![],
            shadow_pdp_ids: vec![],
            merge_strategy: "override".to_string(),
            failure_behavior: PdpFailureBehavior::Deny,
            timeout_ms: 1000,
            max_retries: 0,
            circuit_breaker_threshold: 5,
            cooldown_secs: 30,
            last_known_good_ttl_secs: None,
        };
        let val = serde_json::to_value(&route)?;
        store.upsert_route(tenant, default_route_id, &val).await?;
    }

    Ok(())
}

#[async_trait::async_trait]
impl ObservabilityStore for SqliteStore {
    async fn insert_observation_event(&self, event: &AgentObservationEvent) -> Result<()> {
        let payload = serde_json::to_string(event)?;
        let conn_arc = self.conn.clone();

        let event_id = event.event_id.clone();
        let tenant_id = event.tenant_id.clone();
        let trace_id = event.trace_id.clone();
        let agent_id = event.agent_id.clone();
        let shadow_candidate_id = event.shadow_candidate_id.clone();
        let tool_id = event.tool_id.clone();
        let resource_id = event.resource_id.clone();
        let surface = event.surface.clone();
        let action = event.action.clone();
        let pep_type = event.pep_type.clone();
        let risk_level = event.risk_level.clone();
        let timestamp = event.timestamp.clone();

        tokio::task::spawn_blocking(move || -> Result<()> {
            let conn = conn_arc.lock().unwrap();
            conn.execute(
                r#"
                INSERT INTO observation_events (id, tenant_id, trace_id, agent_id, shadow_candidate_id, tool_id, resource_id, surface, action, pep_type, risk_level, timestamp, payload_json)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                "#,
                params![event_id, tenant_id, trace_id, agent_id, shadow_candidate_id, tool_id, resource_id, surface, action, pep_type, risk_level, timestamp, payload]
            )?;
            Ok(())
        }).await??;
        Ok(())
    }

    async fn list_observation_events(&self, tenant_id: &str) -> Result<Vec<AgentObservationEvent>> {
        let tenant_id = tenant_id.to_string();
        let conn_arc = self.conn.clone();

        let json_strs = tokio::task::spawn_blocking(move || -> Result<Vec<String>> {
            let conn = conn_arc.lock().unwrap();
            let mut stmt = conn.prepare("SELECT payload_json FROM observation_events WHERE tenant_id = ?1 ORDER BY timestamp DESC LIMIT 100")?;
            let mut rows = stmt.query(params![tenant_id])?;
            let mut out = Vec::new();
            while let Some(row) = rows.next()? {
                out.push(row.get(0)?);
            }
            Ok(out)
        }).await??;

        let mut out = Vec::new();
        for j in json_strs {
            if let Ok(e) = serde_json::from_str(&j) {
                out.push(e);
            }
        }
        Ok(out)
    }

    async fn insert_cost_ledger(&self, entry: &CostLedgerEntry) -> Result<()> {
        let conn_arc = self.conn.clone();

        let event_id = entry.event_id.clone();
        let agent_id = entry.agent_id.clone();
        let provider = entry.provider.clone();
        let model = entry.model.clone();
        let input_tokens = entry.input_tokens;
        let output_tokens = entry.output_tokens;
        let total_tokens = entry.total_tokens;
        let input_cost = entry.input_cost;
        let output_cost = entry.output_cost;
        let total_cost = entry.total_cost;
        let currency = entry.currency.clone();
        let estimated = entry.estimated;
        let timestamp = entry.timestamp.clone();

        tokio::task::spawn_blocking(move || -> Result<()> {
            let conn = conn_arc.lock().unwrap();
            conn.execute(
                r#"
                INSERT INTO cost_ledger (id, agent_id, provider, model, input_tokens, output_tokens, total_tokens, input_cost, output_cost, total_cost, currency, estimated, timestamp)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                "#,
                params![event_id, agent_id, provider, model, input_tokens, output_tokens, total_tokens, input_cost, output_cost, total_cost, currency, estimated, timestamp]
            )?;
            Ok(())
        }).await??;
        Ok(())
    }

    async fn list_cost_ledger(&self) -> Result<Vec<CostLedgerEntry>> {
        let conn_arc = self.conn.clone();

        let out = tokio::task::spawn_blocking(move || -> Result<Vec<CostLedgerEntry>> {
            let conn = conn_arc.lock().unwrap();
            let mut stmt = conn.prepare("SELECT id, agent_id, provider, model, input_tokens, output_tokens, total_tokens, input_cost, output_cost, total_cost, currency, estimated, timestamp FROM cost_ledger ORDER BY timestamp DESC")?;
            let mut rows = stmt.query(params![])?;
            let mut out = Vec::new();
            while let Some(r) = rows.next()? {
                out.push(CostLedgerEntry {
                    event_id: r.get("id")?,
                    agent_id: r.get("agent_id")?,
                    provider: r.get("provider")?,
                    model: r.get("model")?,
                    input_tokens: r.get("input_tokens")?,
                    output_tokens: r.get("output_tokens")?,
                    total_tokens: r.get("total_tokens")?,
                    input_cost: r.get("input_cost")?,
                    output_cost: r.get("output_cost")?,
                    total_cost: r.get("total_cost")?,
                    currency: r.get("currency")?,
                    estimated: r.get("estimated")?,
                    timestamp: r.get("timestamp")?,
                });
            }
            Ok(out)
        }).await??;

        Ok(out)
    }

    async fn upsert_policy_suggestion(&self, suggestion: &PolicySuggestion) -> Result<()> {
        let payload = serde_json::to_string(suggestion)?;
        let conn_arc = self.conn.clone();

        let suggestion_id = suggestion.suggestion_id.clone();
        let tenant_id = suggestion.tenant_id.clone();
        let target_agent_id = suggestion.target_agent_id.clone();
        let target_resource_id = suggestion.target_resource_id.clone();
        let suggestion_type = format!("{:?}", suggestion.suggestion_type);
        let status = format!("{:?}", suggestion.status);
        let created_at = suggestion.created_at.clone();

        tokio::task::spawn_blocking(move || -> Result<()> {
            let conn = conn_arc.lock().unwrap();
            conn.execute(
                r#"
                INSERT INTO policy_suggestions (id, tenant_id, target_agent_id, target_resource_id, suggestion_type, status, created_at, data_json)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ON CONFLICT(id) DO UPDATE SET
                    status=excluded.status,
                    data_json=excluded.data_json
                "#,
                params![suggestion_id, tenant_id, target_agent_id, target_resource_id, suggestion_type, status, created_at, payload]
            )?;
            Ok(())
        }).await??;
        Ok(())
    }

    async fn list_policy_suggestions(&self, tenant_id: &str) -> Result<Vec<PolicySuggestion>> {
        let tenant_id = tenant_id.to_string();
        let conn_arc = self.conn.clone();

        let json_strs = tokio::task::spawn_blocking(move || -> Result<Vec<String>> {
            let conn = conn_arc.lock().unwrap();
            let mut stmt = conn.prepare("SELECT data_json FROM policy_suggestions WHERE tenant_id = ?1 ORDER BY created_at DESC")?;
            let mut rows = stmt.query(params![tenant_id])?;
            let mut out = Vec::new();
            while let Some(row) = rows.next()? {
                out.push(row.get(0)?);
            }
            Ok(out)
        }).await??;

        let mut out = Vec::new();
        for j in json_strs {
            if let Ok(s) = serde_json::from_str(&j) {
                out.push(s);
            }
        }
        Ok(out)
    }
}
