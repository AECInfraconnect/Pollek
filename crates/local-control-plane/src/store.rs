use anyhow::Result;
use dek_agent_observer::model::{AgentObservationEvent, CostLedgerEntry};
use dek_control_plane_api::registry::*;
use dek_policy_suggester::model::PolicySuggestion;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::sync::Arc;

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
    pool: SqlitePool,
}

impl SqliteStore {
    pub async fn new(db_url: &str) -> Result<Self> {
        let pool = SqlitePool::connect(db_url).await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
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

        sqlx::query(
            r#"
            INSERT INTO registry_objects (tenant_id, object_type, object_id, status, source, data_json, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
            ON CONFLICT(tenant_id, object_type, object_id) DO UPDATE SET
                status=excluded.status,
                source=excluded.source,
                data_json=excluded.data_json,
                updated_at=excluded.updated_at
            "#
        )
        .bind(tenant_id)
        .bind(object_type)
        .bind(object_id)
        .bind(status)
        .bind(source)
        .bind(json_data)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_object<T: for<'de> Deserialize<'de>>(
        &self,
        tenant_id: &str,
        object_type: &str,
        object_id: &str,
    ) -> Result<Option<T>> {
        let row = sqlx::query("SELECT data_json FROM registry_objects WHERE tenant_id = ?1 AND object_type = ?2 AND object_id = ?3")
            .bind(tenant_id)
            .bind(object_type)
            .bind(object_id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = row {
            let data_json: String = row.try_get("data_json")?;
            let obj: T = serde_json::from_str(&data_json)?;
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
        let rows = sqlx::query(
            "SELECT data_json FROM registry_objects WHERE tenant_id = ?1 AND object_type = ?2",
        )
        .bind(tenant_id)
        .bind(object_type)
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();
        for row in rows {
            let data_json: String = row.try_get("data_json")?;
            let obj: T = serde_json::from_str(&data_json)?;
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
        let result = sqlx::query(
            "DELETE FROM registry_objects WHERE tenant_id = ?1 AND object_type = ?2 AND object_id = ?3",
        )
        .bind(tenant_id)
        .bind(object_type)
        .bind(object_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
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
        sqlx::query(
            r#"
            INSERT INTO registry_objects (tenant_id, object_type, object_id, status, source, data_json, created_at, updated_at)
            VALUES (?1, ?2, ?3, 'raw', 'raw', ?4, ?5, ?5)
            ON CONFLICT(tenant_id, object_type, object_id) DO UPDATE SET
                data_json=excluded.data_json,
                updated_at=excluded.updated_at
            "#,
        )
        .bind(tenant_id)
        .bind(object_type)
        .bind(object_id)
        .bind(&json_data)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_raw(
        &self,
        tenant_id: &str,
        object_type: &str,
        object_id: &str,
    ) -> Result<Option<serde_json::Value>> {
        let row = sqlx::query(
            "SELECT data_json FROM registry_objects WHERE tenant_id = ?1 AND object_type = ?2 AND object_id = ?3",
        )
        .bind(tenant_id)
        .bind(object_type)
        .bind(object_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(r) = row {
            let json_str: String = r.try_get("data_json")?;
            let data: serde_json::Value = serde_json::from_str(&json_str)?;
            Ok(Some(data))
        } else {
            Ok(None)
        }
    }

    async fn list_raw(&self, tenant_id: &str, object_type: &str) -> Result<Vec<serde_json::Value>> {
        let rows = sqlx::query(
            "SELECT data_json FROM registry_objects WHERE tenant_id = ?1 AND object_type = ?2",
        )
        .bind(tenant_id)
        .bind(object_type)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::new();
        for r in rows {
            let json_str: String = r.try_get("data_json")?;
            if let Ok(data) = serde_json::from_str(&json_str) {
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
        sqlx::query(
            "INSERT INTO bundle_blobs (tenant_id, path, bytes) VALUES (?1, ?2, ?3) ON CONFLICT(tenant_id, path) DO UPDATE SET bytes=excluded.bytes"
        )
        .bind(tenant)
        .bind(path)
        .bind(bytes)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_blob(&self, tenant: &str, path: &str) -> Result<Option<Vec<u8>>> {
        let row = sqlx::query("SELECT bytes FROM bundle_blobs WHERE tenant_id = ?1 AND path = ?2")
            .bind(tenant)
            .bind(path)
            .fetch_optional(&self.pool)
            .await?;
        if let Some(row) = row {
            let bytes: Vec<u8> = row.try_get("bytes")?;
            Ok(Some(bytes))
        } else {
            Ok(None)
        }
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
        sqlx::query(
            "INSERT INTO telemetry_events (tenant_id, event_type, event_id, data_json, created_at)
             VALUES (?1,?2,?3,?4,?5)
             ON CONFLICT(tenant_id,event_id) DO UPDATE SET data_json=?4",
        )
        .bind(tenant)
        .bind(kind)
        .bind(event_id)
        .bind(serde_json::to_string(data)?)
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }
    async fn list_telemetry(&self, tenant: &str, kind: &str) -> Result<Vec<serde_json::Value>> {
        let rows = sqlx::query(
            "SELECT data_json FROM telemetry_events WHERE tenant_id=?1 AND event_type=?2 ORDER BY created_at DESC LIMIT 1000")
            .bind(tenant).bind(kind).fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .filter_map(|r| {
                let j: String = r.try_get("data_json").ok()?;
                serde_json::from_str(&j).ok()
            })
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

        sqlx::query(
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
            "#
        )
        .bind(tenant)
        .bind(id)
        .bind(name)
        .bind(category)
        .bind(kind)
        .bind(enabled)
        .bind(status)
        .bind(endpoint)
        .bind(auth_ref)
        .bind(capabilities_json)
        .bind(health_json)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_runtime(&self, tenant: &str, id: &str) -> Result<Option<serde_json::Value>> {
        let row = sqlx::query("SELECT * FROM pdp_runtimes WHERE tenant_id = ?1 AND id = ?2")
            .bind(tenant)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row {
            Ok(Some(Self::row_to_pdp_runtime(r)?))
        } else {
            Ok(None)
        }
    }

    async fn list_runtimes(&self, tenant: &str) -> Result<Vec<serde_json::Value>> {
        let rows = sqlx::query("SELECT * FROM pdp_runtimes WHERE tenant_id = ?1")
            .bind(tenant)
            .fetch_all(&self.pool)
            .await?;

        let mut results = Vec::new();
        for r in rows {
            if let Ok(val) = Self::row_to_pdp_runtime(r) {
                results.push(val);
            }
        }
        Ok(results)
    }

    async fn delete_runtime(&self, tenant: &str, id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM pdp_runtimes WHERE tenant_id = ?1 AND id = ?2")
            .bind(tenant)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
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

        sqlx::query(
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
            "#
        )
        .bind(tenant)
        .bind(id)
        .bind(name)
        .bind(enabled)
        .bind(priority)
        .bind(match_cond_json)
        .bind(mode)
        .bind(primary_pdp_id)
        .bind(fallback_pdp_ids_json)
        .bind(shadow_pdp_ids_json)
        .bind(merge_strategy)
        .bind(failure_behavior)
        .bind(timeout_ms)
        .bind(max_retries)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_route(&self, tenant: &str, id: &str) -> Result<Option<serde_json::Value>> {
        let row = sqlx::query("SELECT * FROM pdp_routes WHERE tenant_id = ?1 AND id = ?2")
            .bind(tenant)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row {
            Ok(Some(Self::row_to_pdp_route(r)?))
        } else {
            Ok(None)
        }
    }

    async fn list_routes(&self, tenant: &str) -> Result<Vec<serde_json::Value>> {
        let rows =
            sqlx::query("SELECT * FROM pdp_routes WHERE tenant_id = ?1 ORDER BY priority DESC")
                .bind(tenant)
                .fetch_all(&self.pool)
                .await?;

        let mut results = Vec::new();
        for r in rows {
            if let Ok(val) = Self::row_to_pdp_route(r) {
                results.push(val);
            }
        }
        Ok(results)
    }

    async fn delete_route(&self, tenant: &str, id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM pdp_routes WHERE tenant_id = ?1 AND id = ?2")
            .bind(tenant)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

impl SqliteStore {
    fn row_to_pdp_runtime(row: sqlx::sqlite::SqliteRow) -> Result<serde_json::Value> {
        let id: String = row.try_get("id")?;
        let name: String = row.try_get("name")?;
        let category: String = row.try_get("category")?;
        let kind: String = row.try_get("kind")?;
        let enabled: bool = row.try_get("enabled")?;
        let status: String = row.try_get("status")?;
        let endpoint: Option<String> = row.try_get("endpoint")?;
        let auth_ref: Option<String> = row.try_get("auth_ref")?;
        let capabilities_json: String = row.try_get("capabilities_json")?;
        let health_json: Option<String> = row.try_get("health_json")?;
        let created_at: String = row.try_get("created_at")?;
        let updated_at: String = row.try_get("updated_at")?;

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

    fn row_to_pdp_route(row: sqlx::sqlite::SqliteRow) -> Result<serde_json::Value> {
        let id: String = row.try_get("id")?;
        let name: String = row.try_get("name")?;
        let enabled: bool = row.try_get("enabled")?;
        let priority: i64 = row.try_get("priority")?;
        let match_cond_json: String = row.try_get("match_cond_json")?;
        let mode: String = row.try_get("mode")?;
        let primary_pdp_id: String = row.try_get("primary_pdp_id")?;
        let fallback_pdp_ids_json: String = row.try_get("fallback_pdp_ids_json")?;
        let shadow_pdp_ids_json: String = row.try_get("shadow_pdp_ids_json")?;
        let merge_strategy: String = row.try_get("merge_strategy")?;
        let failure_behavior: String = row.try_get("failure_behavior")?;
        let timeout_ms: i64 = row.try_get("timeout_ms")?;
        let max_retries: i64 = row.try_get("max_retries")?;
        let created_at: String = row.try_get("created_at")?;
        let updated_at: String = row.try_get("updated_at")?;

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
            enabled: true,
            status: PdpStatus::Ready,
            endpoint: None,
            auth_ref: None,
            capabilities: vec![],
            health: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        },
        PdpRuntime {
            id: "cedar_local".to_string(),
            name: "Cedar Local".to_string(),
            category: PdpRuntimeCategory::LocalEngine,
            kind: PdpKind::CedarLocal,
            enabled: true,
            status: PdpStatus::Ready,
            endpoint: None,
            auth_ref: None,
            capabilities: vec![],
            health: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        },
        PdpRuntime {
            id: "wasm_plugin".to_string(),
            name: "WASM Plugin".to_string(),
            category: PdpRuntimeCategory::LocalEngine,
            kind: PdpKind::WasmPlugin,
            enabled: true,
            status: PdpStatus::Ready,
            endpoint: None,
            auth_ref: None,
            capabilities: vec![],
            health: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        },
        PdpRuntime {
            id: "policy_router".to_string(),
            name: "Policy Router".to_string(),
            category: PdpRuntimeCategory::LocalEngine,
            kind: PdpKind::CustomHttp,
            enabled: true,
            status: PdpStatus::Ready,
            endpoint: None,
            auth_ref: None,
            capabilities: vec![],
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
        sqlx::query(
            r#"
            INSERT INTO observation_events (id, tenant_id, trace_id, agent_id, shadow_candidate_id, tool_id, resource_id, surface, action, pep_type, risk_level, timestamp, payload_json)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            "#
        )
        .bind(&event.event_id)
        .bind(&event.tenant_id)
        .bind(&event.trace_id)
        .bind(&event.agent_id)
        .bind(&event.shadow_candidate_id)
        .bind(&event.tool_id)
        .bind(&event.resource_id)
        .bind(&event.surface)
        .bind(&event.action)
        .bind(&event.pep_type)
        .bind(&event.risk_level)
        .bind(&event.timestamp)
        .bind(&payload)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn list_observation_events(&self, tenant_id: &str) -> Result<Vec<AgentObservationEvent>> {
        let rows = sqlx::query("SELECT payload_json FROM observation_events WHERE tenant_id = ?1 ORDER BY timestamp DESC LIMIT 100")
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await?;
        let mut out = Vec::new();
        for r in rows {
            let j: String = r.try_get("payload_json")?;
            if let Ok(e) = serde_json::from_str(&j) {
                out.push(e);
            }
        }
        Ok(out)
    }

    async fn insert_cost_ledger(&self, entry: &CostLedgerEntry) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO cost_ledger (id, agent_id, provider, model, input_tokens, output_tokens, total_tokens, input_cost, output_cost, total_cost, currency, estimated, timestamp)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            "#
        )
        .bind(&entry.event_id)
        .bind(&entry.agent_id)
        .bind(&entry.provider)
        .bind(&entry.model)
        .bind(entry.input_tokens)
        .bind(entry.output_tokens)
        .bind(entry.total_tokens)
        .bind(entry.input_cost)
        .bind(entry.output_cost)
        .bind(entry.total_cost)
        .bind(&entry.currency)
        .bind(entry.estimated)
        .bind(&entry.timestamp)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn list_cost_ledger(&self) -> Result<Vec<CostLedgerEntry>> {
        let rows = sqlx::query("SELECT id, agent_id, provider, model, input_tokens, output_tokens, total_tokens, input_cost, output_cost, total_cost, currency, estimated, timestamp FROM cost_ledger ORDER BY timestamp DESC")
            .fetch_all(&self.pool)
            .await?;
        let mut out = Vec::new();
        for r in rows {
            out.push(CostLedgerEntry {
                event_id: r.try_get("id")?,
                agent_id: r.try_get("agent_id")?,
                provider: r.try_get("provider")?,
                model: r.try_get("model")?,
                input_tokens: r.try_get("input_tokens")?,
                output_tokens: r.try_get("output_tokens")?,
                total_tokens: r.try_get("total_tokens")?,
                input_cost: r.try_get("input_cost")?,
                output_cost: r.try_get("output_cost")?,
                total_cost: r.try_get("total_cost")?,
                currency: r.try_get("currency")?,
                estimated: r.try_get("estimated")?,
                timestamp: r.try_get("timestamp")?,
            });
        }
        Ok(out)
    }

    async fn upsert_policy_suggestion(&self, suggestion: &PolicySuggestion) -> Result<()> {
        let payload = serde_json::to_string(suggestion)?;
        sqlx::query(
            r#"
            INSERT INTO policy_suggestions (id, tenant_id, target_agent_id, target_resource_id, suggestion_type, status, created_at, data_json)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(id) DO UPDATE SET
                status=excluded.status,
                data_json=excluded.data_json
            "#
        )
        .bind(&suggestion.suggestion_id)
        .bind(&suggestion.tenant_id)
        .bind(&suggestion.target_agent_id)
        .bind(&suggestion.target_resource_id)
        .bind(format!("{:?}", suggestion.suggestion_type))
        .bind(format!("{:?}", suggestion.status))
        .bind(&suggestion.created_at)
        .bind(&payload)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn list_policy_suggestions(&self, tenant_id: &str) -> Result<Vec<PolicySuggestion>> {
        let rows = sqlx::query("SELECT data_json FROM policy_suggestions WHERE tenant_id = ?1 ORDER BY created_at DESC")
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await?;
        let mut out = Vec::new();
        for r in rows {
            let j: String = r.try_get("data_json")?;
            if let Ok(s) = serde_json::from_str(&j) {
                out.push(s);
            }
        }
        Ok(out)
    }
}
