#![allow(clippy::unwrap_used, clippy::needless_borrow)]
use anyhow::Result;
use dek_agent_observer::model::{AgentObservationEvent, CostLedgerEntry};
use dek_agent_observer::usage_budget::AiBudgetLimit;
use dek_agent_observer::usage_model::AiUsageEventV1;
use dek_control_plane_api::registry::*;
use dek_policy_suggester::model::PolicySuggestion;
use rusqlite::{params, params_from_iter, Connection, ToSql};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[async_trait::async_trait]
pub trait DeploymentStore: Send + Sync {
    async fn upsert_deployment_session(
        &self,
        session: dek_domain_schema::deployment_session::DeploymentSession,
    ) -> Result<dek_domain_schema::deployment_session::DeploymentSession>;
    async fn get_deployment_session(
        &self,
        deployment_id: &str,
    ) -> Result<Option<dek_domain_schema::deployment_session::DeploymentSession>>;
    async fn insert_deployment_event(
        &self,
        event: dek_domain_schema::deployment_session::DeploymentEvent,
    ) -> Result<()>;
    async fn list_deployment_events(
        &self,
        deployment_id: &str,
    ) -> Result<Vec<dek_domain_schema::deployment_session::DeploymentEvent>>;
}

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
    async fn delete_raw(&self, tenant_id: &str, object_type: &str, object_id: &str)
        -> Result<bool>;
    async fn clear_raw(&self, tenant_id: &str, object_type: &str) -> Result<u64>;

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
    async fn clear_telemetry(&self, tenant: &str, kind: &str) -> Result<u64>;
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
    async fn clear_observation_events(&self, tenant_id: &str) -> Result<u64>;
    async fn insert_cost_ledger(&self, entry: &CostLedgerEntry) -> Result<()>;
    async fn list_cost_ledger(&self) -> Result<Vec<CostLedgerEntry>>;
    async fn insert_ai_usage_event(&self, event: &AiUsageEventV1) -> Result<()>;
    async fn list_ai_usage_events(&self, query: AiUsageQuery) -> Result<Vec<AiUsageEventV1>>;
    async fn ai_usage_summary(&self, query: AiUsageSummaryQuery) -> Result<AiUsageSummary>;
    async fn upsert_ai_usage_rollup(&self, event: &AiUsageEventV1) -> Result<()>;
    async fn list_ai_budgets(&self, tenant_id: &str) -> Result<Vec<AiBudgetLimit>>;
    async fn upsert_ai_budget(&self, budget: &AiBudgetLimit) -> Result<()>;
    async fn mark_ai_usage_events_sync_status(
        &self,
        event_ids: &[String],
        status: &str,
    ) -> Result<()>;
    async fn upsert_policy_suggestion(&self, suggestion: &PolicySuggestion) -> Result<()>;
    async fn list_policy_suggestions(&self, tenant_id: &str) -> Result<Vec<PolicySuggestion>>;

    // Aggregation queries
    async fn cost_breakdown_by_agent(&self, tenant: &str, since: &str)
        -> Result<Vec<AgentCostRow>>;
    async fn tool_usage_by_agent(&self, tenant: &str, since: &str) -> Result<Vec<ToolUsageRow>>;
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AiUsageQuery {
    pub tenant_id: String,
    pub from: Option<String>,
    pub to: Option<String>,
    pub agent_id: Option<String>,
    pub agent_type: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub task_id: Option<String>,
    pub session_id: Option<String>,
    pub surface: Option<String>,
    pub sync_status: Option<String>,
    pub limit: Option<i64>,
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AiUsageSummaryQuery {
    pub tenant_id: String,
    pub from: Option<String>,
    pub to: Option<String>,
    pub bucket: Option<String>,
    pub agent_id: Option<String>,
    pub agent_type: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub task_id: Option<String>,
    pub session_id: Option<String>,
    pub surface: Option<String>,
    pub group_by: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AiUsageTotals {
    pub request_count: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cached_input_tokens: i64,
    pub cache_write_input_tokens: i64,
    pub reasoning_output_tokens: i64,
    pub tool_tokens: i64,
    pub multimodal_tokens: i64,
    pub total_tokens: i64,
    pub total_cost: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AiUsageBudgetStatus {
    pub window: String,
    pub hard_cost_limit: Option<f64>,
    pub hard_token_limit: Option<i64>,
    pub remaining_cost: Option<f64>,
    pub remaining_tokens: Option<i64>,
    pub status: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AiUsageBreakdown {
    pub key: String,
    pub label: String,
    pub agent_type: Option<String>,
    pub request_count: i64,
    pub total_tokens: i64,
    pub total_cost: f64,
    pub budget: Option<AiUsageBudgetStatus>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AiUsageSeriesPoint {
    pub bucket_start: String,
    pub request_count: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cached_input_tokens: i64,
    pub reasoning_output_tokens: i64,
    pub total_tokens: i64,
    pub total_cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiUsageSummary {
    pub schema_version: String,
    pub tenant_id: String,
    pub from: Option<String>,
    pub to: Option<String>,
    pub bucket: String,
    pub currency: String,
    pub totals: AiUsageTotals,
    pub by_agent: Vec<AiUsageBreakdown>,
    pub by_provider: Vec<AiUsageBreakdown>,
    pub by_model: Vec<AiUsageBreakdown>,
    pub series: Vec<AiUsageSeriesPoint>,
    pub budgets: Vec<AiBudgetLimit>,
}

fn serde_string<T: Serialize>(value: &T) -> Result<String> {
    let raw = serde_json::to_string(value)?;
    Ok(raw.trim_matches('"').to_string())
}

fn option_key(value: &Option<String>) -> String {
    value.clone().unwrap_or_default()
}

fn bucket_start(timestamp: chrono::DateTime<chrono::Utc>, bucket: &str) -> String {
    let seconds = match bucket {
        "5m" => 300,
        "1h" => 3_600,
        "1d" => 86_400,
        _ => 60,
    };
    let epoch = timestamp.timestamp();
    let start = epoch - epoch.rem_euclid(seconds);
    chrono::DateTime::from_timestamp(start, 0)
        .unwrap_or(timestamp)
        .to_rfc3339()
}

fn breakdown_status(
    key: &str,
    budgets: &[AiBudgetLimit],
    cost: f64,
    tokens: i64,
) -> Option<AiUsageBudgetStatus> {
    let budget = budgets
        .iter()
        .find(|budget| budget.enabled && budget.scope_type == "agent" && budget.scope_id == key)?;
    let remaining_cost = budget.hard_cost_limit.map(|limit| limit - cost);
    let remaining_tokens = budget.hard_token_limit.map(|limit| limit - tokens);
    let status = if remaining_cost.map(|value| value <= 0.0).unwrap_or(false)
        || remaining_tokens.map(|value| value <= 0).unwrap_or(false)
    {
        "hard_exceeded"
    } else if budget
        .soft_cost_limit
        .map(|limit| cost >= limit)
        .unwrap_or(false)
        || budget
            .soft_token_limit
            .map(|limit| tokens >= limit)
            .unwrap_or(false)
    {
        "soft_exceeded"
    } else {
        "ok"
    };

    Some(AiUsageBudgetStatus {
        window: budget.window.clone(),
        hard_cost_limit: budget.hard_cost_limit,
        hard_token_limit: budget.hard_token_limit,
        remaining_cost,
        remaining_tokens,
        status: status.to_string(),
    })
}

fn add_usage_to_totals(totals: &mut AiUsageTotals, event: &AiUsageEventV1) {
    totals.request_count += 1;
    totals.input_tokens += event.tokens.input_tokens;
    totals.output_tokens += event.tokens.output_tokens;
    totals.cached_input_tokens += event.tokens.cached_input_tokens;
    totals.cache_write_input_tokens += event.tokens.cache_write_input_tokens;
    totals.reasoning_output_tokens += event.tokens.reasoning_output_tokens;
    totals.tool_tokens += event.tokens.tool_prompt_tokens + event.tokens.tool_result_tokens;
    totals.multimodal_tokens += event.tokens.image_input_tokens
        + event.tokens.image_output_tokens
        + event.tokens.audio_input_tokens
        + event.tokens.audio_output_tokens
        + event.tokens.video_input_tokens;
    totals.total_tokens += event.tokens.total_tokens;
    totals.total_cost += event.cost.total_cost;
}

fn add_usage_to_breakdown(
    map: &mut std::collections::BTreeMap<String, AiUsageBreakdown>,
    key: String,
    label: String,
    agent_type: Option<String>,
    event: &AiUsageEventV1,
) {
    let row = map.entry(key.clone()).or_insert_with(|| AiUsageBreakdown {
        key,
        label,
        agent_type,
        request_count: 0,
        total_tokens: 0,
        total_cost: 0.0,
        budget: None,
    });
    row.request_count += 1;
    row.total_tokens += event.tokens.total_tokens;
    row.total_cost += event.cost.total_cost;
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
            include_str!("../migrations/20260623000000_observability_v2.sql"),
            include_str!("../migrations/20260623000000_resource_access_ledger.sql"),
            include_str!("../migrations/20260624000000_deployment_sessions.sql"),
            include_str!("../migrations/20260626000000_ai_usage_cost_v2.sql"),
        ];

        let tx = conn.transaction()?;
        tx.execute(
            "CREATE TABLE IF NOT EXISTS _migrations (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
            [],
        )?;

        let legacy_table_exists: i64 = tx.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='observation_events'",
            [],
            |row| row.get(0),
        )?;

        let migration_count: i64 =
            tx.query_row("SELECT count(*) FROM _migrations", [], |row| row.get(0))?;
        if legacy_table_exists > 0 && migration_count == 0 {
            // Existing DB without _migrations table, assume migrations 0 to 6 are already applied.
            for i in 0..7 {
                tx.execute(
                    "INSERT INTO _migrations (id, name) VALUES (?1, ?2)",
                    rusqlite::params![i as i64, format!("mig_{}", i)],
                )?;
            }
        }

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
            let conn = conn_arc.lock().unwrap(); //
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
            let conn = conn_arc.lock().unwrap(); //
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
            let conn = conn_arc.lock().unwrap(); //
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
            let conn = conn_arc.lock().unwrap(); //
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
    async fn delete_raw(
        &self,
        tenant_id: &str,
        object_type: &str,
        object_id: &str,
    ) -> Result<bool> {
        self.delete_object(tenant_id, object_type, object_id).await
    }

    async fn clear_raw(&self, tenant_id: &str, object_type: &str) -> Result<u64> {
        let tenant_id = tenant_id.to_string();
        let object_type = object_type.to_string();
        let conn_arc = self.conn.clone();
        let count = tokio::task::spawn_blocking(move || -> Result<usize> {
            let conn = conn_arc.lock().unwrap(); //
            Ok(conn.execute(
                "DELETE FROM registry_objects WHERE tenant_id = ?1 AND object_type = ?2",
                params![tenant_id, object_type],
            )?)
        })
        .await??;
        Ok(count as u64)
    }

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
            let conn = conn_arc.lock().unwrap(); //
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
            let conn = conn_arc.lock().unwrap(); //
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
            let conn = conn_arc.lock().unwrap(); //
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
            let conn = conn_arc.lock().unwrap(); //
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
            let conn = conn_arc.lock().unwrap(); //
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
            let conn = conn_arc.lock().unwrap(); //
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
            let conn = conn_arc.lock().unwrap(); //
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
            let conn = conn_arc.lock().unwrap(); //
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
            let conn = conn_arc.lock().unwrap(); //
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
            let conn = conn_arc.lock().unwrap(); //
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
            let conn = conn_arc.lock().unwrap(); //
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
            let conn = conn_arc.lock().unwrap(); //
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
            let conn = conn_arc.lock().unwrap(); //
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
            let conn = conn_arc.lock().unwrap(); //
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
            let conn = conn_arc.lock().unwrap(); //
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
            let conn = conn_arc.lock().unwrap(); //
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

    async fn clear_telemetry(&self, tenant: &str, kind: &str) -> Result<u64> {
        let tenant = tenant.to_string();
        let kind = kind.to_string();
        let conn_arc = self.conn.clone();
        let count = tokio::task::spawn_blocking(move || -> Result<usize> {
            let conn = conn_arc.lock().unwrap(); //
            Ok(conn.execute(
                "DELETE FROM telemetry_events WHERE tenant_id = ?1 AND event_type = ?2",
                params![tenant, kind],
            )?)
        })
        .await??;
        Ok(count as u64)
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
            let conn = conn_arc.lock().unwrap(); //
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
            let conn = conn_arc.lock().unwrap(); //
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
            let conn = conn_arc.lock().unwrap(); //
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
            let conn = conn_arc.lock().unwrap(); //
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
            let conn = conn_arc.lock().unwrap(); //
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
            let conn = conn_arc.lock().unwrap(); //
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
            let conn = conn_arc.lock().unwrap(); //
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
            let conn = conn_arc.lock().unwrap(); //
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
    async fn clear_observation_events(&self, tenant_id: &str) -> Result<u64> {
        let tenant_id = tenant_id.to_string();
        let conn_arc = self.conn.clone();
        let count = tokio::task::spawn_blocking(move || -> Result<usize> {
            let conn = conn_arc.lock().unwrap(); //
            Ok(conn.execute(
                "DELETE FROM observation_events WHERE tenant_id = ?1",
                params![tenant_id],
            )?)
        })
        .await??;
        Ok(count as u64)
    }

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

        // new fields
        let event_kind = serde_json::to_string(&event.event_kind)
            .unwrap_or_else(|_| "\"generic\"".into())
            .replace("\"", "");
        let provider = event.provider.clone();
        let input_tokens = event.token_usage.as_ref().and_then(|u| u.input_tokens);
        let output_tokens = event.token_usage.as_ref().and_then(|u| u.output_tokens);
        let total_tokens = event.token_usage.as_ref().and_then(|u| u.total_tokens);
        let latency_ms = event.latency_ms;

        tokio::task::spawn_blocking(move || -> Result<()> {
            let conn = conn_arc.lock().unwrap(); //
            conn.execute(
                r#"
                INSERT INTO observation_events (
                    id, tenant_id, trace_id, agent_id, shadow_candidate_id, tool_id, resource_id,
                    surface, action, pep_type, risk_level, timestamp, payload_json,
                    event_kind, provider, input_tokens, output_tokens, total_tokens, latency_ms
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
                "#,
                params![event_id, tenant_id, trace_id, agent_id, shadow_candidate_id, tool_id, resource_id, surface, action, pep_type, risk_level, timestamp, payload, event_kind, provider, input_tokens, output_tokens, total_tokens, latency_ms]
            )?;
            Ok(())
        }).await??;
        Ok(())
    }

    async fn list_observation_events(&self, tenant_id: &str) -> Result<Vec<AgentObservationEvent>> {
        let tenant_id = tenant_id.to_string();
        let conn_arc = self.conn.clone();

        let json_strs = tokio::task::spawn_blocking(move || -> Result<Vec<String>> {
            let conn = conn_arc.lock().unwrap(); //
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
            let conn = conn_arc.lock().unwrap(); //
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
            let conn = conn_arc.lock().unwrap(); //
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

    async fn insert_ai_usage_event(&self, event: &AiUsageEventV1) -> Result<()> {
        let event = event.clone().finalize();
        let conn_arc = self.conn.clone();
        let event_json = serde_json::to_string(&event)?;
        let event_kind = serde_string(&event.event_kind)?;
        let agent_type = serde_string(&event.agent_type)?;
        let usage_source = serde_string(&event.tokens.source)?;
        let cost_source = serde_string(&event.cost.cost_source)?;
        let policy_ids_json = serde_json::to_string(&event.policy_ids)?;
        let usage_details_json = serde_json::to_string(&event.tokens.usage_details_ext)?;
        let cost_details_json = serde_json::to_string(&event.cost.cost_details_ext)?;
        let provider_usage_raw_json = serde_json::to_string(&event.provider_usage_raw)?;
        let metadata_json = serde_json::to_string(&event.metadata)?;
        let usage_estimated = if event.tokens.estimated { 1_i64 } else { 0_i64 };
        let cost_estimated = if event.cost.estimated { 1_i64 } else { 0_i64 };

        tokio::task::spawn_blocking(move || -> Result<()> {
            let conn = conn_arc.lock().unwrap(); //
            conn.execute(
                r#"
                INSERT OR IGNORE INTO ai_usage_events (
                    event_id, schema_version, event_kind, occurred_at, received_at,
                    tenant_id, workspace_id, device_id, actor_id_hash, actor_kind,
                    trace_id, span_id, parent_span_id, session_id, task_id,
                    agent_run_id, agent_step_id, invocation_id,
                    agent_id, agent_instance_id, agent_type, parent_agent_id,
                    subagent_id, shadow_candidate_id,
                    provider, provider_api, provider_request_id, model, model_version,
                    service_tier, inference_region, surface, pep_type, control_mode,
                    policy_ids_json,
                    input_tokens, output_tokens, total_tokens, cached_input_tokens,
                    cache_write_input_tokens, reasoning_output_tokens, tool_prompt_tokens,
                    tool_result_tokens, image_input_tokens, image_output_tokens,
                    audio_input_tokens, audio_output_tokens, usage_source, usage_estimated,
                    usage_details_json,
                    currency, input_cost, output_cost, cached_input_cost,
                    cache_write_input_cost, reasoning_output_cost, tool_cost,
                    image_cost, audio_cost, total_cost, price_catalog_version,
                    pricing_tier_id, cost_source, cost_estimated, cost_details_json,
                    tool_id, tool_name, mcp_server_id, resource_id, resource_type,
                    latency_ms, status, error_code,
                    provider_usage_raw_json, metadata_json, event_json,
                    idempotency_key, cloud_sync_status, local_sequence
                )
                VALUES (
                    ?1, ?2, ?3, ?4, ?5,
                    ?6, ?7, ?8, ?9, ?10,
                    ?11, ?12, ?13, ?14, ?15,
                    ?16, ?17, ?18,
                    ?19, ?20, ?21, ?22,
                    ?23, ?24,
                    ?25, ?26, ?27, ?28, ?29,
                    ?30, ?31, ?32, ?33, ?34,
                    ?35,
                    ?36, ?37, ?38, ?39,
                    ?40, ?41, ?42,
                    ?43, ?44, ?45,
                    ?46, ?47, ?48, ?49,
                    ?50,
                    ?51, ?52, ?53, ?54,
                    ?55, ?56, ?57,
                    ?58, ?59, ?60, ?61,
                    ?62, ?63, ?64, ?65,
                    ?66, ?67, ?68, ?69, ?70,
                    ?71, ?72, ?73,
                    ?74, ?75, ?76,
                    ?77, ?78, ?79
                )
                "#,
                params![
                    event.event_id,
                    event.schema_version,
                    event_kind,
                    event.occurred_at.to_rfc3339(),
                    event.received_at.to_rfc3339(),
                    event.tenant_id,
                    event.workspace_id,
                    event.device_id,
                    event.actor_id_hash,
                    event.actor_kind,
                    event.trace_id,
                    event.span_id,
                    event.parent_span_id,
                    event.session_id,
                    event.task_id,
                    event.agent_run_id,
                    event.agent_step_id,
                    event.invocation_id,
                    event.agent_id,
                    event.agent_instance_id,
                    agent_type,
                    event.parent_agent_id,
                    event.subagent_id,
                    event.shadow_candidate_id,
                    event.provider,
                    event.provider_api,
                    event.provider_request_id,
                    event.model,
                    event.model_version,
                    event.service_tier,
                    event.inference_region,
                    event.surface,
                    event.pep_type,
                    event.control_mode,
                    policy_ids_json,
                    event.tokens.input_tokens,
                    event.tokens.output_tokens,
                    event.tokens.total_tokens,
                    event.tokens.cached_input_tokens,
                    event.tokens.cache_write_input_tokens,
                    event.tokens.reasoning_output_tokens,
                    event.tokens.tool_prompt_tokens,
                    event.tokens.tool_result_tokens,
                    event.tokens.image_input_tokens,
                    event.tokens.image_output_tokens,
                    event.tokens.audio_input_tokens,
                    event.tokens.audio_output_tokens,
                    usage_source,
                    usage_estimated,
                    usage_details_json,
                    event.cost.currency,
                    event.cost.input_cost,
                    event.cost.output_cost,
                    event.cost.cached_input_cost,
                    event.cost.cache_write_input_cost,
                    event.cost.reasoning_output_cost,
                    event.cost.tool_cost,
                    event.cost.image_cost,
                    event.cost.audio_cost,
                    event.cost.total_cost,
                    event.cost.price_catalog_version,
                    event.cost.pricing_tier_id,
                    cost_source,
                    cost_estimated,
                    cost_details_json,
                    event.tool_id,
                    event.tool_name,
                    event.mcp_server_id,
                    event.resource_id,
                    event.resource_type,
                    event.latency_ms,
                    event.status,
                    event.error_code,
                    provider_usage_raw_json,
                    metadata_json,
                    event_json,
                    event.idempotency_key,
                    event
                        .cloud_sync_status
                        .unwrap_or_else(|| "pending".to_string()),
                    event.local_sequence,
                ],
            )?;
            Ok(())
        })
        .await??;

        Ok(())
    }

    async fn list_ai_usage_events(&self, query: AiUsageQuery) -> Result<Vec<AiUsageEventV1>> {
        let conn_arc = self.conn.clone();
        let events = tokio::task::spawn_blocking(move || -> Result<Vec<AiUsageEventV1>> {
            let conn = conn_arc.lock().unwrap(); //
            let mut sql =
                String::from("SELECT event_json FROM ai_usage_events WHERE tenant_id = ?");
            let mut values: Vec<Box<dyn ToSql>> = vec![Box::new(query.tenant_id)];
            if let Some(from) = query.from {
                sql.push_str(" AND occurred_at >= ?");
                values.push(Box::new(from));
            }
            if let Some(to) = query.to {
                sql.push_str(" AND occurred_at <= ?");
                values.push(Box::new(to));
            }
            if let Some(agent_id) = query.agent_id {
                sql.push_str(" AND agent_id = ?");
                values.push(Box::new(agent_id));
            }
            if let Some(agent_type) = query.agent_type {
                sql.push_str(" AND agent_type = ?");
                values.push(Box::new(agent_type));
            }
            if let Some(provider) = query.provider {
                sql.push_str(" AND provider = ?");
                values.push(Box::new(provider));
            }
            if let Some(model) = query.model {
                sql.push_str(" AND model = ?");
                values.push(Box::new(model));
            }
            if let Some(task_id) = query.task_id {
                sql.push_str(" AND task_id = ?");
                values.push(Box::new(task_id));
            }
            if let Some(session_id) = query.session_id {
                sql.push_str(" AND session_id = ?");
                values.push(Box::new(session_id));
            }
            if let Some(surface) = query.surface {
                sql.push_str(" AND surface = ?");
                values.push(Box::new(surface));
            }
            if let Some(sync_status) = query.sync_status {
                sql.push_str(" AND cloud_sync_status = ?");
                values.push(Box::new(sync_status));
            }
            if let Some(cursor) = query.cursor {
                sql.push_str(" AND occurred_at < ?");
                values.push(Box::new(cursor));
            }
            sql.push_str(" ORDER BY occurred_at DESC LIMIT ?");
            values.push(Box::new(query.limit.unwrap_or(100).clamp(1, 10_000)));

            let mut stmt = conn.prepare(&sql)?;
            let params_iter = values.iter().map(|value| value.as_ref() as &dyn ToSql);
            let mut rows = stmt.query(params_from_iter(params_iter))?;
            let mut out = Vec::new();
            while let Some(row) = rows.next()? {
                let event_json: String = row.get(0)?;
                if let Ok(event) = serde_json::from_str::<AiUsageEventV1>(&event_json) {
                    out.push(event);
                }
            }
            Ok(out)
        })
        .await??;

        Ok(events)
    }

    async fn ai_usage_summary(&self, query: AiUsageSummaryQuery) -> Result<AiUsageSummary> {
        let events = self
            .list_ai_usage_events(AiUsageQuery {
                tenant_id: query.tenant_id.clone(),
                from: query.from.clone(),
                to: query.to.clone(),
                agent_id: query.agent_id.clone(),
                agent_type: query.agent_type.clone(),
                provider: query.provider.clone(),
                model: query.model.clone(),
                task_id: query.task_id.clone(),
                session_id: query.session_id.clone(),
                surface: query.surface.clone(),
                limit: Some(10_000),
                ..AiUsageQuery::default()
            })
            .await?;
        let budgets = self
            .list_ai_budgets(&query.tenant_id)
            .await
            .unwrap_or_default();
        let bucket = query.bucket.clone().unwrap_or_else(|| "1m".to_string());
        let mut totals = AiUsageTotals::default();
        let mut by_agent = std::collections::BTreeMap::new();
        let mut by_provider = std::collections::BTreeMap::new();
        let mut by_model = std::collections::BTreeMap::new();
        let mut series = std::collections::BTreeMap::<String, AiUsageSeriesPoint>::new();
        let mut currency = "USD".to_string();

        for event in &events {
            currency = event.cost.currency.clone();
            add_usage_to_totals(&mut totals, event);
            let agent_key = event
                .agent_id
                .clone()
                .or_else(|| event.shadow_candidate_id.clone())
                .unwrap_or_else(|| "unknown".to_string());
            let agent_type = serde_string(&event.agent_type).ok();
            add_usage_to_breakdown(
                &mut by_agent,
                agent_key.clone(),
                agent_key,
                agent_type,
                event,
            );
            let provider_key = event
                .provider
                .clone()
                .unwrap_or_else(|| "unknown".to_string());
            add_usage_to_breakdown(
                &mut by_provider,
                provider_key.clone(),
                provider_key,
                None,
                event,
            );
            let model_key = event.model.clone().unwrap_or_else(|| "unknown".to_string());
            add_usage_to_breakdown(&mut by_model, model_key.clone(), model_key, None, event);

            let bucket_key = bucket_start(event.occurred_at, &bucket);
            let point = series
                .entry(bucket_key.clone())
                .or_insert_with(|| AiUsageSeriesPoint {
                    bucket_start: bucket_key,
                    ..AiUsageSeriesPoint::default()
                });
            point.request_count += 1;
            point.input_tokens += event.tokens.input_tokens;
            point.output_tokens += event.tokens.output_tokens;
            point.cached_input_tokens += event.tokens.cached_input_tokens;
            point.reasoning_output_tokens += event.tokens.reasoning_output_tokens;
            point.total_tokens += event.tokens.total_tokens;
            point.total_cost += event.cost.total_cost;
        }

        let mut by_agent: Vec<_> = by_agent.into_values().collect();
        for row in &mut by_agent {
            row.budget = breakdown_status(&row.key, &budgets, row.total_cost, row.total_tokens);
        }
        by_agent.sort_by(|left, right| {
            right
                .total_cost
                .partial_cmp(&left.total_cost)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut by_provider: Vec<_> = by_provider.into_values().collect();
        by_provider.sort_by(|left, right| {
            right
                .total_cost
                .partial_cmp(&left.total_cost)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut by_model: Vec<_> = by_model.into_values().collect();
        by_model.sort_by(|left, right| {
            right
                .total_cost
                .partial_cmp(&left.total_cost)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(AiUsageSummary {
            schema_version: "ai-usage-summary.v1".to_string(),
            tenant_id: query.tenant_id,
            from: query.from,
            to: query.to,
            bucket,
            currency,
            totals,
            by_agent,
            by_provider,
            by_model,
            series: series.into_values().collect(),
            budgets,
        })
    }

    async fn upsert_ai_usage_rollup(&self, event: &AiUsageEventV1) -> Result<()> {
        let conn_arc = self.conn.clone();
        let event = event.clone();
        let bucket_start = bucket_start(event.occurred_at, "1m");
        let agent_id_key = option_key(&event.agent_id);
        let provider_key = option_key(&event.provider);
        let model_key = option_key(&event.model);
        let surface_key = event.surface.clone();
        let tool_id_key = option_key(&event.tool_id);
        let resource_id_key = option_key(&event.resource_id);
        let agent_type = serde_string(&event.agent_type)?;

        tokio::task::spawn_blocking(move || -> Result<()> {
            let conn = conn_arc.lock().unwrap(); //
            conn.execute(
                r#"
                INSERT INTO ai_usage_rollups (
                    bucket_start, bucket_size, tenant_id, workspace_id, device_id,
                    agent_id, agent_id_key, agent_type, provider, provider_key,
                    model, model_key, surface, surface_key, tool_id, tool_id_key,
                    resource_id, resource_id_key, request_count,
                    input_tokens, output_tokens, total_tokens, cached_input_tokens,
                    cache_write_input_tokens, reasoning_output_tokens, total_cost, currency
                )
                VALUES (?1, '1m', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, 1, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25)
                ON CONFLICT(
                    bucket_start, bucket_size, tenant_id, agent_id_key, provider_key,
                    model_key, surface_key, tool_id_key, resource_id_key
                ) DO UPDATE SET
                    request_count=request_count + 1,
                    input_tokens=input_tokens + excluded.input_tokens,
                    output_tokens=output_tokens + excluded.output_tokens,
                    total_tokens=total_tokens + excluded.total_tokens,
                    cached_input_tokens=cached_input_tokens + excluded.cached_input_tokens,
                    cache_write_input_tokens=cache_write_input_tokens + excluded.cache_write_input_tokens,
                    reasoning_output_tokens=reasoning_output_tokens + excluded.reasoning_output_tokens,
                    total_cost=total_cost + excluded.total_cost,
                    currency=excluded.currency
                "#,
                params![
                    bucket_start,
                    event.tenant_id,
                    event.workspace_id,
                    event.device_id,
                    event.agent_id,
                    agent_id_key,
                    agent_type,
                    event.provider,
                    provider_key,
                    event.model,
                    model_key,
                    event.surface,
                    surface_key,
                    event.tool_id,
                    tool_id_key,
                    event.resource_id,
                    resource_id_key,
                    event.tokens.input_tokens,
                    event.tokens.output_tokens,
                    event.tokens.total_tokens,
                    event.tokens.cached_input_tokens,
                    event.tokens.cache_write_input_tokens,
                    event.tokens.reasoning_output_tokens,
                    event.cost.total_cost,
                    event.cost.currency,
                ],
            )?;
            Ok(())
        })
        .await??;
        Ok(())
    }

    async fn list_ai_budgets(&self, tenant_id: &str) -> Result<Vec<AiBudgetLimit>> {
        let tenant_id = tenant_id.to_string();
        let conn_arc = self.conn.clone();
        let rows = tokio::task::spawn_blocking(move || -> Result<Vec<String>> {
            let conn = conn_arc.lock().unwrap(); //
            let mut stmt = conn.prepare(
                "SELECT data_json FROM ai_budget_limits WHERE tenant_id = ?1 ORDER BY updated_at DESC",
            )?;
            let mut rows = stmt.query(params![tenant_id])?;
            let mut out = Vec::new();
            while let Some(row) = rows.next()? {
                out.push(row.get(0)?);
            }
            Ok(out)
        })
        .await??;
        let mut budgets = Vec::new();
        for row in rows {
            if let Ok(budget) = serde_json::from_str(&row) {
                budgets.push(budget);
            }
        }
        Ok(budgets)
    }

    async fn upsert_ai_budget(&self, budget: &AiBudgetLimit) -> Result<()> {
        let budget = budget.clone();
        let payload = serde_json::to_string(&budget)?;
        let enabled = if budget.enabled { 1_i64 } else { 0_i64 };
        let conn_arc = self.conn.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let conn = conn_arc.lock().unwrap(); //
            conn.execute(
                r#"
                INSERT INTO ai_budget_limits (
                    budget_id, tenant_id, scope_type, scope_id, window, currency,
                    soft_cost_limit, hard_cost_limit, soft_token_limit, hard_token_limit,
                    action_on_soft, action_on_hard, enabled, created_at, updated_at, data_json
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
                ON CONFLICT(budget_id) DO UPDATE SET
                    tenant_id=excluded.tenant_id,
                    scope_type=excluded.scope_type,
                    scope_id=excluded.scope_id,
                    window=excluded.window,
                    currency=excluded.currency,
                    soft_cost_limit=excluded.soft_cost_limit,
                    hard_cost_limit=excluded.hard_cost_limit,
                    soft_token_limit=excluded.soft_token_limit,
                    hard_token_limit=excluded.hard_token_limit,
                    action_on_soft=excluded.action_on_soft,
                    action_on_hard=excluded.action_on_hard,
                    enabled=excluded.enabled,
                    updated_at=excluded.updated_at,
                    data_json=excluded.data_json
                "#,
                params![
                    budget.budget_id,
                    budget.tenant_id,
                    budget.scope_type,
                    budget.scope_id,
                    budget.window,
                    budget.currency,
                    budget.soft_cost_limit,
                    budget.hard_cost_limit,
                    budget.soft_token_limit,
                    budget.hard_token_limit,
                    budget.action_on_soft,
                    budget.action_on_hard,
                    enabled,
                    budget.created_at,
                    budget.updated_at,
                    payload,
                ],
            )?;
            Ok(())
        })
        .await??;
        Ok(())
    }

    async fn mark_ai_usage_events_sync_status(
        &self,
        event_ids: &[String],
        status: &str,
    ) -> Result<()> {
        if event_ids.is_empty() {
            return Ok(());
        }
        let ids = event_ids.to_vec();
        let status = status.to_string();
        let conn_arc = self.conn.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut conn = conn_arc.lock().unwrap(); //
            let tx = conn.transaction()?;
            for event_id in ids {
                tx.execute(
                    "UPDATE ai_usage_events SET cloud_sync_status = ?1 WHERE event_id = ?2",
                    params![status, event_id],
                )?;
            }
            tx.commit()?;
            Ok(())
        })
        .await??;
        Ok(())
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
            let conn = conn_arc.lock().unwrap(); //
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
            let conn = conn_arc.lock().unwrap(); //
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

    async fn cost_breakdown_by_agent(
        &self,
        _tenant: &str,
        since: &str,
    ) -> Result<Vec<AgentCostRow>> {
        let since_val = since.to_string();
        let conn_arc = self.conn.clone();

        let rows = tokio::task::spawn_blocking(move || -> Result<Vec<AgentCostRow>> {
            let conn = conn_arc.lock().unwrap(); //
            let sql = r#"
                SELECT agent_id,
                       COALESCE(SUM(total_cost),0)   AS cost,
                       COALESCE(SUM(total_tokens),0) AS tokens
                FROM cost_ledger
                WHERE timestamp >= ?1
                GROUP BY agent_id
                ORDER BY cost DESC
            "#;
            let mut stmt = conn.prepare(sql)?;
            let mut rows = stmt.query(params![since_val])?;
            let mut result = Vec::new();
            while let Some(row) = rows.next()? {
                result.push(AgentCostRow {
                    agent_id: row.get(0)?,
                    cost: row.get(1)?,
                    tokens: row.get(2)?,
                });
            }
            Ok(result)
        })
        .await??;
        Ok(rows)
    }

    async fn tool_usage_by_agent(&self, tenant: &str, since: &str) -> Result<Vec<ToolUsageRow>> {
        let tenant_val = tenant.to_string();
        let since_val = since.to_string();
        let conn_arc = self.conn.clone();

        let rows = tokio::task::spawn_blocking(move || -> Result<Vec<ToolUsageRow>> {
            let conn = conn_arc.lock().unwrap(); //
            let sql = r#"
                SELECT agent_id, tool_id,
                       COUNT(*) AS calls,
                       SUM(CASE WHEN json_extract(payload_json,'$.decision.allow')=0 THEN 1 ELSE 0 END) AS denied,
                       AVG(latency_ms) AS avg_latency
                FROM observation_events
                WHERE tenant_id=?1 AND event_kind='tool_call' AND timestamp>=?2
                GROUP BY agent_id, tool_id
                ORDER BY calls DESC
            "#;
            let mut stmt = conn.prepare(sql)?;
            let mut rows = stmt.query(params![tenant_val, since_val])?;
            let mut result = Vec::new();
            while let Some(row) = rows.next()? {
                result.push(ToolUsageRow {
                    agent_id: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    tool_id: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    calls: row.get(2)?,
                    denied: row.get(3)?,
                    avg_latency: row.get::<_, Option<f64>>(4)?.unwrap_or(0.0),
                });
            }
            Ok(result)
        }).await??;
        Ok(rows)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentCostRow {
    pub agent_id: String,
    pub cost: f64,
    pub tokens: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolUsageRow {
    pub agent_id: String,
    pub tool_id: String,
    pub calls: i64,
    pub denied: i64,
    pub avg_latency: f64,
}

#[async_trait::async_trait]
impl DeploymentStore for SqliteStore {
    async fn upsert_deployment_session(
        &self,
        session: dek_domain_schema::deployment_session::DeploymentSession,
    ) -> Result<dek_domain_schema::deployment_session::DeploymentSession> {
        let conn = self.conn.clone();
        let session_clone = session.clone();
        tokio::task::spawn_blocking(
            move || -> Result<dek_domain_schema::deployment_session::DeploymentSession> {
                let mut conn = conn
                    .lock()
                    .map_err(|e| anyhow::anyhow!("lock failed: {}", e))?;
                let tx = conn.transaction()?;

                let status_str = serde_json::to_string(&session_clone.status)?
                    .trim_matches('"')
                    .to_string();
                let target_scope_json = serde_json::to_string(&session_clone.target_scope)?;

                let mut stmt = tx.prepare(
                    "INSERT INTO deployment_sessions (
                    deployment_id, policy_id, policy_version, requested_control_level,
                    target_scope_json, status, created_by, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                ON CONFLICT(deployment_id) DO UPDATE SET
                    status=excluded.status,
                    target_scope_json=excluded.target_scope_json,
                    updated_at=excluded.updated_at",
                )?;

                let requested_control_level_str =
                    serde_json::to_string(&session_clone.requested_control_level)?
                        .trim_matches('"')
                        .to_string();

                stmt.execute(params![
                    session_clone.deployment_id,
                    session_clone.policy_id,
                    session_clone.policy_version,
                    requested_control_level_str,
                    target_scope_json,
                    status_str,
                    session_clone.created_by,
                    session_clone.created_at.to_rfc3339(),
                    session_clone.updated_at.to_rfc3339()
                ])?;
                drop(stmt);

                tx.commit()?;
                Ok(session_clone)
            },
        )
        .await?
    }

    async fn get_deployment_session(
        &self,
        deployment_id: &str,
    ) -> Result<Option<dek_domain_schema::deployment_session::DeploymentSession>> {
        let conn = self.conn.clone();
        let deployment_id = deployment_id.to_string();

        tokio::task::spawn_blocking(
            move || -> Result<Option<dek_domain_schema::deployment_session::DeploymentSession>> {
                let conn = conn
                    .lock()
                    .map_err(|e| anyhow::anyhow!("lock failed: {}", e))?;
                let mut stmt =
                    conn.prepare("SELECT * FROM deployment_sessions WHERE deployment_id = ?1")?;
                let mut rows = stmt.query(params![deployment_id])?;

                if let Some(r) = rows.next()? {
                    let status_str: String = r.get("status")?;
                    let req_level_str: String = r.get("requested_control_level")?;

                    let session = dek_domain_schema::deployment_session::DeploymentSession {
                        deployment_id: r.get("deployment_id")?,
                        policy_id: r.get("policy_id")?,
                        policy_version: r.get("policy_version")?,
                        requested_control_level: serde_json::from_str(&format!(
                            "\"{}\"",
                            req_level_str
                        ))
                        .unwrap_or(dek_domain_schema::control_level::ControlLevel::Observe),
                        target_scope: serde_json::from_str(
                            &r.get::<_, String>("target_scope_json")?,
                        )?,
                        status: serde_json::from_str(&format!("\"{}\"", status_str))?,
                        created_at: chrono::DateTime::parse_from_rfc3339(
                            &r.get::<_, String>("created_at")?,
                        )?
                        .with_timezone(&chrono::Utc),
                        updated_at: chrono::DateTime::parse_from_rfc3339(
                            &r.get::<_, String>("updated_at")?,
                        )?
                        .with_timezone(&chrono::Utc),
                        created_by: r.get("created_by")?,
                    };
                    Ok(Some(session))
                } else {
                    Ok(None)
                }
            },
        )
        .await?
    }

    async fn insert_deployment_event(
        &self,
        event: dek_domain_schema::deployment_session::DeploymentEvent,
    ) -> Result<()> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut conn = conn.lock().map_err(|e| anyhow::anyhow!("lock failed: {}", e))?;
            let tx = conn.transaction()?;

            let phase_str = serde_json::to_string(&event.phase)?.trim_matches('"').to_string();
            let status_str = serde_json::to_string(&event.status)?.trim_matches('"').to_string();
            let title_json = serde_json::to_string(&event.title)?;
            let detail_json = serde_json::to_string(&event.detail)?;
            let tech_detail_json = event.technical_detail.as_ref().map(serde_json::to_string).transpose()?;
            let user_action_json = event.user_action.as_ref().map(serde_json::to_string).transpose()?;

            let mut stmt = tx.prepare(
                "INSERT INTO deployment_events (
                    event_id, deployment_id, agent_id, entity_id, policy_id, phase, status,
                    title_json, detail_json, technical_detail_json, user_action_json, created_at, correlation_id
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"
            )?;

            stmt.execute(params![
                event.event_id,
                event.deployment_id,
                event.agent_id,
                event.entity_id,
                event.policy_id,
                phase_str,
                status_str,
                title_json,
                detail_json,
                tech_detail_json,
                user_action_json,
                event.created_at.to_rfc3339(),
                event.correlation_id
            ])?;
            drop(stmt);

            tx.commit()?;
            Ok(())
        }).await?
    }

    async fn list_deployment_events(
        &self,
        deployment_id: &str,
    ) -> Result<Vec<dek_domain_schema::deployment_session::DeploymentEvent>> {
        let conn = self.conn.clone();
        let deployment_id = deployment_id.to_string();

        tokio::task::spawn_blocking(move || -> Result<Vec<dek_domain_schema::deployment_session::DeploymentEvent>> {
            let conn = conn.lock().map_err(|e| anyhow::anyhow!("lock failed: {}", e))?;
            let mut stmt = conn.prepare("SELECT * FROM deployment_events WHERE deployment_id = ?1 ORDER BY created_at ASC")?;
            let mut rows = stmt.query(params![deployment_id])?;

            let mut events = Vec::new();
            while let Some(r) = rows.next()? {
                let phase_str: String = r.get("phase")?;
                let status_str: String = r.get("status")?;

                let title: dek_domain_schema::deployment_session::LocalizedText = serde_json::from_str(&r.get::<_, String>("title_json")?)?;
                let detail: dek_domain_schema::deployment_session::LocalizedText = serde_json::from_str(&r.get::<_, String>("detail_json")?)?;
                let tech_detail = r.get::<_, Option<String>>("technical_detail_json")?.map(|s| serde_json::from_str(&s)).transpose()?;
                let user_action = r.get::<_, Option<String>>("user_action_json")?.map(|s| serde_json::from_str(&s)).transpose()?;

                let event = dek_domain_schema::deployment_session::DeploymentEvent {
                    event_id: r.get("event_id")?,
                    deployment_id: r.get("deployment_id")?,
                    agent_id: r.get("agent_id")?,
                    entity_id: r.get("entity_id")?,
                    policy_id: r.get("policy_id")?,
                    phase: serde_json::from_str(&format!("\"{}\"", phase_str))?,
                    status: serde_json::from_str(&format!("\"{}\"", status_str))?,
                    title,
                    detail,
                    technical_detail: tech_detail,
                    user_action,
                    created_at: chrono::DateTime::parse_from_rfc3339(&r.get::<_, String>("created_at")?)?.with_timezone(&chrono::Utc),
                    correlation_id: r.get("correlation_id")?,
                };
                events.push(event);
            }
            Ok(events)
        }).await?
    }
}
