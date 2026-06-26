pub mod generated {
    #![allow(clippy::all)]
    #![allow(clippy::unwrap_used)]
    #![allow(non_camel_case_types)]
    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}
pub use generated::*;

pub const CONTRACT_VERSION: &str = "1.0";
pub const BUNDLE_ENVELOPE_SCHEMA_VERSION: &str = "bundle-envelope.v1";
pub const TELEMETRY_ENVELOPE_SCHEMA_VERSION: &str = "telemetry-envelope.v1";

#[derive(Debug, thiserror::Error)]
pub enum ContractError {
    #[error("unsupported contract version: {0}")]
    UnsupportedVersion(String),

    #[error("schema mismatch: {0}")]
    SchemaMismatch(String),

    #[error("missing capability: {0}")]
    MissingCapability(String),
}
