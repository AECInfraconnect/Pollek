use super::*;

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
