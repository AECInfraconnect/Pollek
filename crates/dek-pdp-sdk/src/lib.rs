// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

//! Plugin SDK for Pollen DEK.
//!
//! This SDK provides the necessary traits and structures to build and register custom
//! Policy Decision Point (PDP) adapters for the Pollen DEK runtime.
//!
//! # Guardrails for Custom Adapters
//!
//! 1. **No Local Authoring/Compilation**: Adapters MUST NOT compile raw policy text on the edge. They should expect compiled artifacts (e.g. Wasm modules, OCI images).
//! 2. **Fail-Closed**: Any failure during initialization or evaluation MUST result in a closed (DENY) decision.
//! 3. **Stateless**: PDPs should evaluate requests statelessly without persisting evaluation data locally.

use std::collections::HashMap;

// Re-export core PDP traits so adapters only need to depend on the SDK
pub use dek_policy_runtime::{PolicyDecision, PolicyError, PolicyRuntime};
use serde_json::Value;
use thiserror::Error;

/// Information about a registered adapter
#[derive(Debug, Clone)]
pub struct AdapterInfo {
    /// The unique identifier of the adapter
    pub id: String,
    /// A short description of what the adapter does
    pub description: String,
    /// Version of the adapter
    pub version: String,
}

impl AdapterInfo {
    pub fn new(
        id: impl Into<String>,
        description: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            version: version.into(),
        }
    }
}

/// Error returned when an adapter fails to build
#[derive(Debug, Error)]
pub enum BuildError {
    #[error("Missing required configuration field: {0}")]
    MissingConfig(String),
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("Initialization failed: {0}")]
    InitFailed(String),
}

/// Factory trait for building instances of `PolicyRuntime`
pub trait AdapterFactory: Send + Sync {
    /// Returns metadata about this adapter
    fn info(&self) -> AdapterInfo;

    /// Builds a new `PolicyRuntime` instance using the provided configuration payload
    fn build(&self, config: &Value) -> Result<Box<dyn PolicyRuntime>, BuildError>;
}

/// Registry for managing available adapter factories
#[derive(Default)]
pub struct AdapterRegistry {
    factories: HashMap<String, Box<dyn AdapterFactory>>,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
        }
    }

    /// Registers a new adapter factory
    pub fn register(&mut self, factory: Box<dyn AdapterFactory>) {
        let info = factory.info();
        self.factories.insert(info.id, factory);
    }

    /// Retrieves an adapter factory by ID
    pub fn get(&self, id: &str) -> Option<&dyn AdapterFactory> {
        self.factories.get(id).map(|b| b.as_ref())
    }

    /// Returns a list of all registered adapter IDs
    pub fn available_ids(&self) -> Vec<String> {
        self.factories.keys().cloned().collect()
    }

    /// Convenience method to build an adapter directly from the registry
    pub fn build_adapter(
        &self,
        id: &str,
        config: &Value,
    ) -> Result<Box<dyn PolicyRuntime>, BuildError> {
        if let Some(factory) = self.get(id) {
            factory.build(config)
        } else {
            Err(BuildError::InitFailed(format!(
                "Adapter '{}' not found",
                id
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use serde_json::json;

    struct DummyRuntime;

    #[async_trait]
    impl PolicyRuntime for DummyRuntime {
        async fn evaluate(&self, _input: serde_json::Value) -> Result<PolicyDecision, PolicyError> {
            Ok(PolicyDecision {
                evaluator_id: "dummy".to_string(),
                evaluator_type: "dummy".to_string(),
                required: true,
                status: "success".to_string(),
                decision: "deny".to_string(),
                allow: false,
                reason: "fail-closed".to_string(),
                effects: json!({}),
                obligations: vec![],
                metadata: json!({}),
            })
        }

        fn version(&self) -> String {
            "dummy-v1".to_string()
        }

        async fn clear_cache(&self) {}
    }

    struct DummyFactory;

    impl AdapterFactory for DummyFactory {
        fn info(&self) -> AdapterInfo {
            AdapterInfo::new("dummy", "A dummy adapter for testing", "1.0.0")
        }

        fn build(&self, config: &Value) -> Result<Box<dyn PolicyRuntime>, BuildError> {
            if config.get("fail").is_some() {
                return Err(BuildError::InvalidConfig("forced failure".into()));
            }
            Ok(Box::new(DummyRuntime))
        }
    }

    #[test]
    fn test_adapter_info() {
        let info = AdapterInfo::new("test", "desc", "v1");
        assert_eq!(info.id, "test");
        assert_eq!(info.description, "desc");
        assert_eq!(info.version, "v1");
    }

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = AdapterRegistry::new();
        registry.register(Box::new(DummyFactory));

        assert!(registry.get("dummy").is_some());
        assert!(registry.get("unknown").is_none());
        assert_eq!(registry.available_ids(), vec!["dummy"]);
    }

    #[test]
    fn test_registry_build() {
        let mut registry = AdapterRegistry::new();
        registry.register(Box::new(DummyFactory));

        let success = registry.build_adapter("dummy", &json!({}));
        assert!(success.is_ok());

        let failure = registry.build_adapter("dummy", &json!({"fail": true}));
        assert!(matches!(failure, Err(BuildError::InvalidConfig(_))));

        let unknown = registry.build_adapter("unknown", &json!({}));
        assert!(matches!(unknown, Err(BuildError::InitFailed(_))));
    }
}
