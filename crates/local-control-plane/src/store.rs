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

mod deployment;
mod observability;
mod pdp;
mod policy;
mod registry;
mod telemetry;

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
    async fn query_observation_events(
        &self,
        query: ObservationEventQuery,
    ) -> Result<Vec<AgentObservationEvent>>;
    async fn clear_observation_events(&self, tenant_id: &str) -> Result<u64>;
    async fn insert_cost_ledger(&self, tenant_id: &str, entry: &CostLedgerEntry) -> Result<()>;
    async fn list_cost_ledger(&self, tenant_id: &str) -> Result<Vec<CostLedgerEntry>>;
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

/// Filtered query over `observation_events`. `agent_ids` matches an event when
/// either its `agent_id` or its `shadow_candidate_id` equals one of the given
/// ids, so discovery candidates whose events were correlated under a shadow id
/// still surface in their per-agent view.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ObservationEventQuery {
    pub tenant_id: String,
    pub agent_ids: Vec<String>,
    pub event_kind: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub limit: Option<i64>,
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
            include_str!("../migrations/20260722000000_cost_ledger_tenant.sql"),
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
            let conn = conn_arc.lock().map_err(|_| anyhow::anyhow!("sqlite store connection lock poisoned"))?;
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
            let conn = conn_arc.lock().map_err(|_| anyhow::anyhow!("sqlite store connection lock poisoned"))?;
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
            let conn = conn_arc
                .lock()
                .map_err(|_| anyhow::anyhow!("sqlite store connection lock poisoned"))?;
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
            let conn = conn_arc.lock().map_err(|_| anyhow::anyhow!("sqlite store connection lock poisoned"))?;
            let changed = conn.execute(
                "DELETE FROM registry_objects WHERE tenant_id = ?1 AND object_type = ?2 AND object_id = ?3",
                params![tenant_id, object_type, object_id],
            )?;
            Ok(changed)
        }).await??;

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
