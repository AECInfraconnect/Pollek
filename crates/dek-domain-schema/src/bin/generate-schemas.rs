use schemars::schema_for;
use std::fs;
use std::path::Path;

use dek_domain_schema::tenant::Tenant;
use dek_domain_schema::agent::Agent;
use dek_domain_schema::entity::Entity;
use dek_domain_schema::resource::Resource;
use dek_domain_schema::relationship::Relationship;
use dek_domain_schema::policy_target::PolicyTarget;
use dek_domain_schema::decision::{DecisionRequest, DecisionResult};
use dek_domain_schema::telemetry::TelemetryEnvelope;
use dek_domain_schema::ebpf::EbpfMapUpdate;
use dek_domain_schema::bundle::BundleManifest;

fn main() {
    let schema_dir = Path::new("../../schemas");
    fs::create_dir_all(schema_dir).unwrap();

    let schemas = vec![
        ("pollen-tenant.schema.json", schema_for!(Tenant)),
        ("pollen-agent.schema.json", schema_for!(Agent)),
        ("pollen-entity.schema.json", schema_for!(Entity)),
        ("pollen-resource.schema.json", schema_for!(Resource)),
        ("pollen-relationship.schema.json", schema_for!(Relationship)),
        ("pollen-policy_target.schema.json", schema_for!(PolicyTarget)),
        ("pollen-decision-request.schema.json", schema_for!(DecisionRequest)),
        ("pollen-decision-result.schema.json", schema_for!(DecisionResult)),
        ("pollen-telemetry-envelope.schema.json", schema_for!(TelemetryEnvelope)),
        ("pollen-ebpf-map-update.schema.json", schema_for!(EbpfMapUpdate)),
        ("pollen-bundle-manifest.schema.json", schema_for!(BundleManifest)),
    ];

    for (filename, schema) in schemas {
        let out_path = schema_dir.join(filename);
        let out_json = serde_json::to_string_pretty(&schema).unwrap();
        fs::write(&out_path, out_json).unwrap();
        println!("Generated {}", out_path.display());
    }
}
