use super::*;

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
