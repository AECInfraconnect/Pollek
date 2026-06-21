// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use dek_auth::{Verifier, VerifierConfig};
use dek_policy_router::PolicyRouter;
use dek_wasm_host::WasmPluginHost;
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct DekMetadata {
    pub tenant_id: String,
    pub device_id: String,
    pub spiffe_id: Option<String>,
    pub jwt_public_key_pem: Option<String>,
    pub jwks: Option<jsonwebtoken::jwk::JwkSet>,
    pub issuer_url: Option<String>,
    pub audience: Option<Vec<String>>,
    pub enterprise_profile: dek_config::EnterpriseProfile,
}

pub struct RuntimeSnapshot {
    pub generation: u64,
    pub bundle_id: String,
    pub bundle_version: u64,
    pub router: Arc<PolicyRouter>,
    pub metadata: DekMetadata,
    pub verifier: Arc<Verifier>,
    pub plugin_host: Arc<WasmPluginHost>,
}

impl RuntimeSnapshot {
    pub fn new(
        generation: u64,
        bundle_id: String,
        bundle_version: u64,
        router: Arc<PolicyRouter>,
        metadata: DekMetadata,
        plugin_host: Arc<WasmPluginHost>,
    ) -> Self {
        let verifier = Verifier::new(VerifierConfig {
            jwks: metadata.jwks.clone(),
            public_key_pem: metadata.jwt_public_key_pem.clone(),
            issuer: metadata.issuer_url.clone(),
            audience: metadata.audience.clone(),
            leeway_secs: 60,
        });

        Self {
            generation,
            bundle_id,
            bundle_version,
            router,
            metadata,
            verifier: Arc::new(verifier),
            plugin_host,
        }
    }
}
