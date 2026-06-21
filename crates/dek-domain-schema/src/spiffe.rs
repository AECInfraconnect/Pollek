// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use crate::tenant::TrustDomainStrategy;
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpiffeId {
    pub trust_domain: String,
    pub path: String,
}

impl SpiffeId {
    pub fn parse(uri: &str) -> Result<Self, &'static str> {
        let parsed = Url::parse(uri).map_err(|_| "Invalid URI format")?;

        if parsed.scheme() != "spiffe" {
            return Err("Must use spiffe:// scheme");
        }

        let trust_domain = parsed.host_str().ok_or("Missing trust domain")?.to_string();
        let path = parsed.path().to_string();

        if path.is_empty() || path == "/" {
            return Err("Missing SPIFFE ID path");
        }

        Ok(Self { trust_domain, path })
    }

    pub fn to_uri(&self) -> String {
        format!("spiffe://{}{}", self.trust_domain, self.path)
    }
}

pub struct SpiffeBuilder {
    strategy: TrustDomainStrategy,
    tenant_id: String,
    global_trust_domain: String,
}

impl SpiffeBuilder {
    pub fn new(
        strategy: TrustDomainStrategy,
        tenant_id: String,
        global_trust_domain: String,
    ) -> Self {
        Self {
            strategy,
            tenant_id,
            global_trust_domain,
        }
    }

    pub fn build_agent_id(&self, agent_id: &str) -> SpiffeId {
        match &self.strategy {
            TrustDomainStrategy::Shared => SpiffeId {
                trust_domain: self.global_trust_domain.clone(),
                path: format!("/tenant/{}/agent/{}", self.tenant_id, agent_id),
            },
            TrustDomainStrategy::Dedicated => SpiffeId {
                trust_domain: format!("{}.{}", self.tenant_id, self.global_trust_domain),
                path: format!("/agent/{}", agent_id),
            },
            TrustDomainStrategy::Custom(domain) => SpiffeId {
                trust_domain: domain.clone(),
                path: format!("/agent/{}", agent_id),
            },
            TrustDomainStrategy::Federated | TrustDomainStrategy::CustomerManaged => SpiffeId {
                trust_domain: String::new(),
                path: String::new(),
            },
        }
    }

    pub fn build_device_id(&self, device_id: &str) -> SpiffeId {
        match &self.strategy {
            TrustDomainStrategy::Shared => SpiffeId {
                trust_domain: self.global_trust_domain.clone(),
                path: format!("/tenant/{}/device/{}", self.tenant_id, device_id),
            },
            TrustDomainStrategy::Dedicated => SpiffeId {
                trust_domain: format!("{}.{}", self.tenant_id, self.global_trust_domain),
                path: format!("/device/{}", device_id),
            },
            TrustDomainStrategy::Custom(domain) => SpiffeId {
                trust_domain: domain.clone(),
                path: format!("/device/{}", device_id),
            },
            TrustDomainStrategy::Federated | TrustDomainStrategy::CustomerManaged => SpiffeId {
                trust_domain: String::new(),
                path: String::new(),
            },
        }
    }
}

pub fn validate_tenant_isolation(
    spiffe: &SpiffeId,
    strategy: &TrustDomainStrategy,
    expected_tenant: &str,
    global_trust_domain: &str,
) -> Result<(), &'static str> {
    match strategy {
        TrustDomainStrategy::Shared => {
            if spiffe.trust_domain != global_trust_domain {
                return Err("Trust domain mismatch for Shared strategy");
            }
            let expected_prefix = format!("/tenant/{}/", expected_tenant);
            if !spiffe.path.starts_with(&expected_prefix) {
                return Err("Tenant path prefix mismatch in Shared strategy");
            }
            Ok(())
        }
        TrustDomainStrategy::Dedicated => {
            let expected_domain = format!("{}.{}", expected_tenant, global_trust_domain);
            if spiffe.trust_domain != expected_domain {
                return Err("Trust domain mismatch for Dedicated strategy");
            }
            Ok(())
        }
        TrustDomainStrategy::Custom(domain) => {
            if spiffe.trust_domain != *domain {
                return Err("Trust domain mismatch for Custom strategy");
            }
            Ok(())
        }
        TrustDomainStrategy::Federated | TrustDomainStrategy::CustomerManaged => {
            Err("Unsupported strategy")
        }
    }
}
