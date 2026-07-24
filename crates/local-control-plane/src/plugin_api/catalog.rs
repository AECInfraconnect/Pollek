//! Plugin marketplace catalog: built-in catalog entries plus local plugin-registry items.
use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::{json, Value};
use std::path::{Path as FsPath, PathBuf};

use super::string_array;
use crate::state::AppState;

pub(super) fn marketplace_items() -> Vec<Value> {
    let mut items = catalog_marketplace_items();
    append_local_registry_items(&mut items);
    items
}

fn catalog_marketplace_items() -> Vec<Value> {
    vec![
        json!({
            "id": "com.pollek.pii-redactor",
            "name": "PII Redactor",
            "version": "1.0.0",
            "kind": "telemetry.transform",
            "publisher": "AEC Infraconnect",
            "verified": true,
            "rating": 4.8,
            "installs": 1280,
            "capabilities": ["telemetry:read", "telemetry:write"],
            "human_capabilities": [
                "Redacts private data from local telemetry before export or display",
                "Does not send data off this device"
            ],
            "os": ["windows", "linux", "macos"],
            "min_engine_version": "1.0.0",
            "signature_ok": true,
            "signature_state": "valid",
            "latest_version": "1.1.0",
            "update_available": true,
            "rollback_supported": true,
            "registry_ref": "local://plugins/com.pollek.pii-redactor/1.1.0",
            "release_notes": "Adds broader local metadata redaction rules and safer output-size limits.",
            "trust_labels": ["verified", "local_only"],
            "lifecycle_state": "update_available",
            "description_en": "Masks common PII fields in activity metadata.",
            "description_th": "Masks common private-data fields in local activity metadata.",
            "privacy_note": "Local transform only. No network access requested.",
            "source": "local_catalog"
        }),
        json!({
            "id": "com.pollek.definition-feed",
            "name": "AI Agent Definition Feed",
            "version": "0.3.0",
            "kind": "definition.feed",
            "publisher": "AEC Infraconnect",
            "verified": true,
            "rating": 4.6,
            "installs": 920,
            "capabilities": ["definitions:write", "candidates:write"],
            "human_capabilities": [
                "Adds well-known AI app signatures and friendly explanations",
                "Improves discovery and observe labels"
            ],
            "os": ["windows", "linux", "macos"],
            "min_engine_version": "1.0.0",
            "signature_ok": true,
            "signature_state": "valid",
            "latest_version": "0.3.0",
            "update_available": false,
            "rollback_supported": true,
            "registry_ref": "local://plugins/com.pollek.definition-feed/0.3.0",
            "release_notes": "Ships curated AI app definitions and observe explanations.",
            "trust_labels": ["verified", "local_only"],
            "lifecycle_state": "available",
            "description_en": "Updates local AI app definitions used by discovery and reference intel.",
            "description_th": "Updates local AI app definitions and friendly explanations.",
            "privacy_note": "Writes local definitions. No native OS capability requested.",
            "source": "local_catalog"
        }),
        json!({
            "id": "com.example.splunk-exporter",
            "name": "Splunk Telemetry Exporter",
            "version": "0.1.0",
            "kind": "telemetry.exporter",
            "publisher": "Example Labs",
            "verified": false,
            "rating": 0.0,
            "installs": 0,
            "capabilities": ["telemetry:read", "http_out:splunk.example.com:443"],
            "human_capabilities": [
                "Reads activity metadata",
                "Sends selected telemetry to splunk.example.com"
            ],
            "os": ["windows", "linux", "macos"],
            "min_engine_version": "1.0.0",
            "signature_ok": false,
            "signature_state": "test_only",
            "latest_version": "0.1.0",
            "update_available": false,
            "rollback_supported": false,
            "registry_ref": "sideload://com.example.splunk-exporter/0.1.0",
            "release_notes": "Developer preview. Use only in Advanced or private test environments.",
            "trust_labels": ["developer_preview", "sends_data_out", "unverified"],
            "lifecycle_state": "available",
            "description_en": "Developer preview exporter for a Splunk HEC endpoint.",
            "description_th": "Developer preview exporter for sending selected telemetry to Splunk.",
            "privacy_note": "This plugin can send activity metadata off this device. Install only for testing.",
            "source": "local_catalog"
        }),
    ]
}

fn append_local_registry_items(items: &mut Vec<Value>) {
    let registry_dir = local_plugin_registry_dir();
    let index_path = registry_dir.join("index.json");
    let Ok(index_bytes) = std::fs::read(&index_path) else {
        return;
    };
    let Ok(index) = serde_json::from_slice::<Value>(&index_bytes) else {
        return;
    };
    let Some(index_items) = index.get("items").and_then(Value::as_array) else {
        return;
    };
    for entry in index_items {
        if let Some(item) = local_registry_marketplace_item(&registry_dir, entry) {
            let id = item.get("id").and_then(Value::as_str).unwrap_or_default();
            items.retain(|existing| existing.get("id").and_then(Value::as_str) != Some(id));
            items.push(item);
        }
    }
}

fn local_plugin_registry_dir() -> PathBuf {
    std::env::var("POLLEK_PLUGIN_REGISTRY_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(
                std::env::var("DEK_LCP_DATA").unwrap_or_else(|_| "./pollek-local-data".into()),
            )
            .join("plugin-registry")
        })
}

fn local_registry_marketplace_item(registry_dir: &FsPath, entry: &Value) -> Option<Value> {
    let relative_path = entry.get("path").and_then(Value::as_str)?;
    let manifest_path = registry_dir
        .join(relative_path.replace('/', std::path::MAIN_SEPARATOR_STR))
        .join("pollek-plugin.json");
    let manifest = std::fs::read(&manifest_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())?;
    let id = manifest
        .get("id")
        .or_else(|| entry.get("id"))
        .and_then(Value::as_str)
        .unwrap_or("local-plugin");
    let version = manifest
        .get("version")
        .or_else(|| entry.get("version"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let signature_state = manifest
        .pointer("/signature/status")
        .or_else(|| entry.get("signature_state"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let capabilities = manifest_capabilities(&manifest);
    Some(json!({
        "id": id,
        "name": manifest.get("name").and_then(Value::as_str).unwrap_or(id),
        "version": version,
        "latest_version": version,
        "kind": manifest.get("kind").and_then(Value::as_str).unwrap_or("unknown"),
        "publisher": manifest.pointer("/author/name").and_then(Value::as_str).unwrap_or("Local registry"),
        "verified": manifest.pointer("/author/verified").and_then(Value::as_bool).unwrap_or(false),
        "rating": 0.0,
        "installs": 0,
        "capabilities": capabilities,
        "human_capabilities": human_capabilities_for_manifest(&manifest),
        "os": string_array(&manifest, "os"),
        "min_engine_version": manifest.get("min_engine_version").and_then(Value::as_str).unwrap_or("unknown"),
        "signature_ok": signature_state == "valid",
        "signature_state": signature_state,
        "update_available": false,
        "rollback_supported": manifest.pointer("/registry/rollback_versions").and_then(Value::as_array).is_some_and(|values| !values.is_empty()),
        "registry_ref": format!("local://plugins/{id}/{version}"),
        "release_notes": "Installed from local plugin registry. Review manifest, checksum, signature, and capabilities before enabling.",
        "trust_labels": manifest.pointer("/governance/trust_labels").cloned().unwrap_or_else(|| json!(["developer_preview", "unverified"])),
        "lifecycle_state": "available",
        "description_en": format!("Local registry plugin: {id}"),
        "description_th": format!("Local registry plugin: {id}"),
        "privacy_note": privacy_note_for_manifest(&manifest),
        "source": manifest.pointer("/registry/source").and_then(Value::as_str).unwrap_or("sideload")
    }))
}

fn manifest_capabilities(manifest: &Value) -> Vec<String> {
    let mut capabilities = Vec::new();
    append_capability_group(manifest, &mut capabilities, "host", "host");
    append_capability_group(manifest, &mut capabilities, "http_out", "http_out");
    append_capability_group(manifest, &mut capabilities, "kv", "kv");
    append_capability_group(manifest, &mut capabilities, "native", "native");
    append_capability_group(manifest, &mut capabilities, "data_scope", "data");
    capabilities
}

fn append_capability_group(manifest: &Value, out: &mut Vec<String>, key: &str, prefix: &str) {
    if let Some(values) = manifest
        .pointer(&format!("/capabilities/{key}"))
        .and_then(Value::as_array)
    {
        for value in values.iter().filter_map(Value::as_str) {
            out.push(format!("{prefix}:{value}"));
        }
    }
}

fn human_capabilities_for_manifest(manifest: &Value) -> Vec<String> {
    manifest_capabilities(manifest)
        .into_iter()
        .map(|capability| {
            capability
                .strip_prefix("http_out:")
                .map(|host| format!("Sends approved data to {host}"))
                .or_else(|| {
                    capability
                        .strip_prefix("native:")
                        .map(|cap| format!("Uses reviewed native capability {cap}"))
                })
                .or_else(|| {
                    capability
                        .strip_prefix("data:")
                        .map(|scope| format!("Accesses Pollek data scope {scope}"))
                })
                .unwrap_or_else(|| capability.replace(':', " "))
        })
        .collect()
}

fn privacy_note_for_manifest(manifest: &Value) -> String {
    if manifest
        .pointer("/capabilities/http_out")
        .and_then(Value::as_array)
        .is_some_and(|values| !values.is_empty())
    {
        "This plugin can send approved metadata off this device. Install only after consent."
            .to_string()
    } else if manifest
        .pointer("/capabilities/native")
        .and_then(Value::as_array)
        .is_some_and(|values| !values.is_empty())
    {
        "This plugin requests native OS capability. Review current OS readiness before enabling."
            .to_string()
    } else {
        "Local registry plugin. No outbound HTTP capability is declared.".to_string()
    }
}

pub(super) async fn list_marketplace_items(
    Path(_tenant): Path<String>,
    State(_state): State<AppState>,
) -> Json<Value> {
    Json(json!({
        "schema_version": "pollek.marketplace.v1",
        "items": marketplace_items()
    }))
}

pub(super) async fn marketplace_item_detail(
    Path((_tenant, id)): Path<(String, String)>,
    State(_state): State<AppState>,
) -> Json<Value> {
    let item = marketplace_items()
        .into_iter()
        .find(|item| item.get("id").and_then(Value::as_str) == Some(id.as_str()));
    Json(json!({
        "schema_version": "pollek.marketplace.item.v1",
        "item": item
    }))
}
