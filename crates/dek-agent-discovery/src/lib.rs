#![deny(clippy::unwrap_used)]

pub mod aggregator;
pub mod api;
pub mod browser_scan;
pub mod cli_agent_scan;
pub mod config;
pub mod config_paths;
pub mod container_scan;
pub mod error;
pub mod fingerprint;
pub mod ide_extension_scan;
pub mod local_model_probe;
pub mod mcp_config;
pub mod mcp_scan;
pub mod model;
pub mod orchestrator;
pub mod process_scan;
pub mod redaction;
pub mod source_catalog;

pub use api::{run_scan, run_scan_v2, to_registry_agent, to_registry_agent_v2};
