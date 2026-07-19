use super::*;

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
            let conn = conn_arc.lock().map_err(|_| anyhow::anyhow!("sqlite store connection lock poisoned"))?;
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
            let conn = conn_arc
                .lock()
                .map_err(|_| anyhow::anyhow!("sqlite store connection lock poisoned"))?;
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
            .unwrap_or("observe");
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
            let conn = conn_arc.lock().map_err(|_| anyhow::anyhow!("sqlite store connection lock poisoned"))?;
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
            let conn = conn_arc.lock().map_err(|_| anyhow::anyhow!("sqlite store connection lock poisoned"))?;
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
            let conn = conn_arc.lock().map_err(|_| anyhow::anyhow!("sqlite store connection lock poisoned"))?;
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
            let conn = conn_arc.lock().map_err(|_| anyhow::anyhow!("sqlite store connection lock poisoned"))?;
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
            let conn = conn_arc
                .lock()
                .map_err(|_| anyhow::anyhow!("sqlite store connection lock poisoned"))?;
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
