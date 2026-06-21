#![allow(clippy::unwrap_used, clippy::expect_used)]
use schemars::schema_for;
use std::fs;
use std::path::Path;

use dek_domain_schema::{
    agent::AiAgent,
    dek_device::DekDevice,
    mcp_server::McpServer,
    pep_deployment::PepDeployment,
    policy::Policy,
    principal::Principal,
    relationship::Relationship,
    resource::Resource,
    telemetry_event::TelemetryEvent,
    tenant::Tenant,
    tool::Tool,
};

fn main() {
    let schema_dir = Path::new("../../schemas");
    fs::create_dir_all(schema_dir).unwrap();

    let schemas = vec![
        ("tenant.schema.json", schema_for!(Tenant)),
        ("principal.schema.json", schema_for!(Principal)),
        ("dek-device.schema.json", schema_for!(DekDevice)),
        ("ai-agent.schema.json", schema_for!(AiAgent)),
        ("mcp-server.schema.json", schema_for!(McpServer)),
        ("tool.schema.json", schema_for!(Tool)),
        ("resource.schema.json", schema_for!(Resource)),
        ("relationship.schema.json", schema_for!(Relationship)),
        ("policy.schema.json", schema_for!(Policy)),
        ("pep-deployment.schema.json", schema_for!(PepDeployment)),
        ("telemetry-event.schema.json", schema_for!(TelemetryEvent)),
    ];

    for (filename, schema) in schemas {
        let out_path = schema_dir.join(filename);
        let out_json = serde_json::to_string_pretty(&schema).unwrap();
        fs::write(&out_path, out_json).unwrap();
        println!("Generated {}", out_path.display());
    }
}
