#![allow(clippy::panic)]
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use std::fs;
use std::path::Path;

use crate::state::AppState;
use dek_domain_schema::*;

pub fn load_seed_data(state: &AppState, profile: &str) {
    tracing::info!("Loading fixtures for profile: {}", profile);
    let base_dir = Path::new("fixtures");
    let fixtures_dir = base_dir.join(profile);
    let default_dir = base_dir;

    let mut reg = state.registry.lock().unwrap();

    let get_path = |name: &str| {
        let p = fixtures_dir.join(name);
        if p.exists() {
            p
        } else {
            default_dir.join(name)
        }
    };

    if let Ok(content) = fs::read_to_string(get_path("tenant.json")) {
        if let Ok(tenant) = serde_json::from_str::<Tenant>(&content) {
            reg.tenants.insert(tenant.tenant_id.clone(), tenant);
        }
    }
    if let Ok(content) = fs::read_to_string(get_path("principal.json")) {
        if let Ok(p) = serde_json::from_str::<Principal>(&content) {
            reg.principals.insert(p.principal_id.clone(), p);
        }
    }
    if let Ok(content) = fs::read_to_string(get_path("dek-device.json")) {
        if let Ok(d) = serde_json::from_str::<DekDevice>(&content) {
            reg.devices.insert(d.device_id.clone(), d);
        }
    }
    if let Ok(content) = fs::read_to_string(get_path("ai-agent.json")) {
        if let Ok(a) = serde_json::from_str::<AiAgent>(&content) {
            reg.agents.insert(a.agent_id.clone(), a);
        }
    }
    if let Ok(content) = fs::read_to_string(get_path("mcp-server.json")) {
        if let Ok(m) = serde_json::from_str::<McpServer>(&content) {
            reg.mcp_servers.insert(m.server_id.clone(), m);
        }
    }
    if let Ok(content) = fs::read_to_string(get_path("tool.json")) {
        if let Ok(t) = serde_json::from_str::<Tool>(&content) {
            reg.tools.insert(t.tool_id.clone(), t);
        }
    }
    if let Ok(content) = fs::read_to_string(get_path("resource.json")) {
        if let Ok(r) = serde_json::from_str::<Resource>(&content) {
            reg.resources.insert(r.resource_id.clone(), r);
        }
    }
    if let Ok(content) = fs::read_to_string(get_path("relationship.json")) {
        if let Ok(rel) = serde_json::from_str::<Relationship>(&content) {
            reg.relationships.push(rel);
        }
    }
    if let Ok(content) = fs::read_to_string(get_path("policy.json")) {
        if let Ok(pol) = serde_json::from_str::<Policy>(&content) {
            reg.policies.insert(pol.policy_id.clone(), pol);
        }
    }
    if let Ok(content) = fs::read_to_string(get_path("pep-deployment.json")) {
        if let Ok(pep) = serde_json::from_str::<PepDeployment>(&content) {
            reg.pep_deployments
                .insert(pep.pep_deployment_id.clone(), pep);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::fs;

    #[test]
    fn validate_all_fixtures() {
        let schemas_dir = Path::new("../../docs/contracts/schemas");
        let fixtures_dir = Path::new("./fixtures");

        let pairs = vec![
            ("tenant.schema.json", "tenant.json"),
            ("principal.schema.json", "principal.json"),
            ("dek-device.schema.json", "dek-device.json"),
            ("ai-agent.schema.json", "ai-agent.json"),
            ("mcp-server.schema.json", "mcp-server.json"),
            ("tool.schema.json", "tool.json"),
            ("resource.schema.json", "resource.json"),
            ("relationship.schema.json", "relationship.json"),
            ("policy.schema.json", "policy.json"),
            ("pep-deployment.schema.json", "pep-deployment.json"),
            ("telemetry-event.schema.json", "telemetry-event.json"),
        ];

        for (schema_name, fixture_name) in pairs {
            let schema_path = schemas_dir.join(schema_name);
            let fixture_path = fixtures_dir.join(fixture_name);

            assert!(
                schema_path.exists(),
                "Schema missing: {}",
                schema_path.display()
            );
            assert!(
                fixture_path.exists(),
                "Fixture missing: {}",
                fixture_path.display()
            );

            let schema_str = fs::read_to_string(&schema_path).unwrap();
            let fixture_str = fs::read_to_string(&fixture_path).unwrap();

            let schema_json: Value = serde_json::from_str(&schema_str).unwrap();
            let fixture_json: Value = serde_json::from_str(&fixture_str).unwrap();

            let compiled = jsonschema::validator_for(&schema_json).expect("Invalid JSON schema");
            if !compiled.is_valid(&fixture_json) {
                panic!(
                    "Fixture {} failed validation against {}",
                    fixture_name, schema_name
                );
            }
        }
    }
}
