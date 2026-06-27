use super::*;

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
