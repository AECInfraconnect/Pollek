use super::*;

#[async_trait::async_trait]
impl ObservabilityStore for SqliteStore {
    async fn clear_observation_events(&self, tenant_id: &str) -> Result<u64> {
        let tenant_id = tenant_id.to_string();
        let conn_arc = self.conn.clone();
        let count = tokio::task::spawn_blocking(move || -> Result<usize> {
            let conn = conn_arc
                .lock()
                .map_err(|_| anyhow::anyhow!("sqlite store connection lock poisoned"))?;
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
            let conn = conn_arc.lock().map_err(|_| anyhow::anyhow!("sqlite store connection lock poisoned"))?;
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
            let conn = conn_arc.lock().map_err(|_| anyhow::anyhow!("sqlite store connection lock poisoned"))?;
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

    async fn query_observation_events(
        &self,
        query: ObservationEventQuery,
    ) -> Result<Vec<AgentObservationEvent>> {
        let conn_arc = self.conn.clone();

        let json_strs = tokio::task::spawn_blocking(move || -> Result<Vec<String>> {
            let conn = conn_arc
                .lock()
                .map_err(|_| anyhow::anyhow!("sqlite store connection lock poisoned"))?;
            let mut sql =
                String::from("SELECT payload_json FROM observation_events WHERE tenant_id = ?");
            let mut values: Vec<Box<dyn ToSql>> = vec![Box::new(query.tenant_id)];
            if !query.agent_ids.is_empty() {
                let placeholders = std::iter::repeat_n("?", query.agent_ids.len())
                    .collect::<Vec<_>>()
                    .join(", ");
                sql.push_str(&format!(
                    " AND (agent_id IN ({placeholders}) OR shadow_candidate_id IN ({placeholders}))"
                ));
                for agent_id in &query.agent_ids {
                    values.push(Box::new(agent_id.clone()));
                }
                for agent_id in query.agent_ids {
                    values.push(Box::new(agent_id));
                }
            }
            if let Some(event_kind) = query.event_kind {
                sql.push_str(" AND event_kind = ?");
                values.push(Box::new(event_kind));
            }
            if let Some(from) = query.from {
                sql.push_str(" AND timestamp >= ?");
                values.push(Box::new(from));
            }
            if let Some(to) = query.to {
                sql.push_str(" AND timestamp <= ?");
                values.push(Box::new(to));
            }
            sql.push_str(" ORDER BY timestamp DESC LIMIT ?");
            values.push(Box::new(query.limit.unwrap_or(200).clamp(1, 5_000)));

            let mut stmt = conn.prepare(&sql)?;
            let params_iter = values.iter().map(|value| value.as_ref() as &dyn ToSql);
            let mut rows = stmt.query(params_from_iter(params_iter))?;
            let mut out = Vec::new();
            while let Some(row) = rows.next()? {
                out.push(row.get(0)?);
            }
            Ok(out)
        })
        .await??;

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
            let conn = conn_arc.lock().map_err(|_| anyhow::anyhow!("sqlite store connection lock poisoned"))?;
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
            let conn = conn_arc.lock().map_err(|_| anyhow::anyhow!("sqlite store connection lock poisoned"))?;
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
            let conn = conn_arc
                .lock()
                .map_err(|_| anyhow::anyhow!("sqlite store connection lock poisoned"))?;
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
            let conn = conn_arc
                .lock()
                .map_err(|_| anyhow::anyhow!("sqlite store connection lock poisoned"))?;
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
            let conn = conn_arc.lock().map_err(|_| anyhow::anyhow!("sqlite store connection lock poisoned"))?;
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
            let conn = conn_arc.lock().map_err(|_| anyhow::anyhow!("sqlite store connection lock poisoned"))?;
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
            let conn = conn_arc
                .lock()
                .map_err(|_| anyhow::anyhow!("sqlite store connection lock poisoned"))?;
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
            let mut conn = conn_arc
                .lock()
                .map_err(|_| anyhow::anyhow!("sqlite store connection lock poisoned"))?;
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
            let conn = conn_arc.lock().map_err(|_| anyhow::anyhow!("sqlite store connection lock poisoned"))?;
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
            let conn = conn_arc.lock().map_err(|_| anyhow::anyhow!("sqlite store connection lock poisoned"))?;
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
            let conn = conn_arc
                .lock()
                .map_err(|_| anyhow::anyhow!("sqlite store connection lock poisoned"))?;
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
            let conn = conn_arc.lock().map_err(|_| anyhow::anyhow!("sqlite store connection lock poisoned"))?;
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
