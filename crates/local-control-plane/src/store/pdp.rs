use super::*;

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
