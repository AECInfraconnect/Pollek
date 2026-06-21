use std::fs;
use std::path::Path;

pub fn load_fixture() {
    tracing::info!("Loaded fixture");
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::fs;

    #[test]
    fn validate_all_fixtures() {
        let schemas_dir = Path::new("../../schemas");
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

            assert!(schema_path.exists(), "Schema missing: {}", schema_path.display());
            assert!(fixture_path.exists(), "Fixture missing: {}", fixture_path.display());

            let schema_str = fs::read_to_string(&schema_path).unwrap();
            let fixture_str = fs::read_to_string(&fixture_path).unwrap();

            let schema_json: Value = serde_json::from_str(&schema_str).unwrap();
            let fixture_json: Value = serde_json::from_str(&fixture_str).unwrap();

            let compiled = jsonschema::validator_for(&schema_json).expect("Invalid JSON schema");
            if !compiled.is_valid(&fixture_json) {
                panic!("Fixture {} failed validation against {}", fixture_name, schema_name);
            }
        }
    }
}
