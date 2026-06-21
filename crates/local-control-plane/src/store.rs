use anyhow::Result;
use dek_control_plane_api::registry::*;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use dek_agent_observer::model::{AgentObservationEvent, CostLedgerEntry};
use dek_policy_suggester::model::PolicySuggestion;

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
pub trait ConnectorStore: Send + Sync {
    async fn upsert(&self, tenant: &str, id: &str, data: &serde_json::Value) -> Result<()>;
    async fn get(&self, tenant: &str, id: &str) -> Result<Option<serde_json::Value>>;
    async fn list(&self, tenant: &str) -> Result<Vec<serde_json::Value>>;
    async fn delete(&self, tenant: &str, id: &str) -> Result<bool>;
}

#[async_trait::async_trait]
pub trait ObservabilityStore: Send + Sync {
    async fn insert_observation_event(&self, event: &AgentObservationEvent) -> Result<()>;
    async fn list_observation_events(&self, tenant_id: &str) -> Result<Vec<AgentObservationEvent>>;
    async fn insert_cost_ledger(&self, entry: &CostLedgerEntry) -> Result<()>;
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
impl ConnectorStore for SqliteStore {
    async fn upsert(&self, tenant: &str, id: &str, data: &serde_json::Value) -> Result<()> {
        self.upsert_object(tenant, "connector", id, "active", "local", data)
            .await
    }

    async fn get(&self, tenant: &str, id: &str) -> Result<Option<serde_json::Value>> {
        self.get_object(tenant, "connector", id).await
    }

    async fn list(&self, tenant: &str) -> Result<Vec<serde_json::Value>> {
        self.list_objects(tenant, "connector").await
    }

    async fn delete(&self, tenant: &str, id: &str) -> Result<bool> {
        self.delete_object(tenant, "connector", id).await
    }
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
        .bind(&suggestion.suggestion_type)
        .bind(&suggestion.status)
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
