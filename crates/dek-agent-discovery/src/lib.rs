#![deny(clippy::unwrap_used)]

pub mod aggregator;
pub mod api;
pub mod browser_scan;
pub mod browser_session_reader;
pub mod browser_window_scan;
pub mod capability_inventory;
pub mod capability_retrieval;
pub mod cli_agent_scan;
pub mod config;
pub mod config_paths;
pub mod container_scan;
pub mod error;
pub mod fingerprint;
pub mod human_loop;
pub mod ide_extension_scan;
pub mod identity;
pub mod identity_hint;
pub mod identity_key;
pub mod installed_app_scan;
pub mod local_model_probe;
pub mod mcp_config;
pub mod mcp_scan;
pub mod model;
pub mod orchestrator;
pub mod process_scan;
pub mod python_framework_scan;
pub mod redaction;
pub mod signature_match;
pub mod sni_source;
pub mod source_catalog;
pub mod web_ai_scan;

pub use api::{run_scan, run_scan_v2, stable_agent_key, to_registry_agent, to_registry_agent_v2};
