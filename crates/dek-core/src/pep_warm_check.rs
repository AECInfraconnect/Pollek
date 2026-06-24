// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use dek_domain_schema::capabilities::PepWarmCheck;
use dek_domain_schema::deployment_session::{EnforcementLayer, RoutingPlan};

pub struct McpProxyWarmCheck {
    base_url: String,
}

impl McpProxyWarmCheck {
    pub fn new(base_url: String) -> Self {
        Self { base_url }
    }
}

impl PepWarmCheck for McpProxyWarmCheck {
    async fn warm_check(&self, _plan: &RoutingPlan) -> Result<(), String> {
        let client = reqwest::Client::new();
        let url = format!("{}/health", self.base_url);
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => Ok(()),
            Ok(resp) => Err(format!("MCP Proxy returned status {}", resp.status())),
            Err(e) => Err(format!("MCP Proxy unreachable: {}", e)),
        }
    }
}

pub struct EbpfNetworkWarmCheck;

impl PepWarmCheck for EbpfNetworkWarmCheck {
    async fn warm_check(&self, _plan: &RoutingPlan) -> Result<(), String> {
        // Mock implementation for eBPF: verify maps are loaded
        Ok(())
    }
}

pub struct WindowsWfpWarmCheck;

impl PepWarmCheck for WindowsWfpWarmCheck {
    async fn warm_check(&self, _plan: &RoutingPlan) -> Result<(), String> {
        // Mock implementation for WFP: verify callouts
        Ok(())
    }
}

pub struct MacosNeFilterWarmCheck;

impl PepWarmCheck for MacosNeFilterWarmCheck {
    async fn warm_check(&self, _plan: &RoutingPlan) -> Result<(), String> {
        // Mock implementation for NEFilter
        Ok(())
    }
}

pub struct McpStdioWrapperWarmCheck;

impl PepWarmCheck for McpStdioWrapperWarmCheck {
    async fn warm_check(&self, _plan: &RoutingPlan) -> Result<(), String> {
        // Mock implementation: ping wrapper binary
        Ok(())
    }
}

pub async fn run_warm_check(plan: &RoutingPlan) -> Result<(), String> {
    match plan.selected_pep.layer {
        EnforcementLayer::McpProxy => {
            McpProxyWarmCheck::new("http://127.0.0.1:4000".into())
                .warm_check(plan)
                .await
        }
        EnforcementLayer::McpStdioWrapper => McpStdioWrapperWarmCheck.warm_check(plan).await,
        EnforcementLayer::EbpfNetwork => EbpfNetworkWarmCheck.warm_check(plan).await,
        EnforcementLayer::WindowsWfp => WindowsWfpWarmCheck.warm_check(plan).await,
        EnforcementLayer::MacosNetworkExtension => MacosNeFilterWarmCheck.warm_check(plan).await,
        EnforcementLayer::ObserveOnly
        | EnforcementLayer::HttpProxy
        | EnforcementLayer::BrowserExtension => Ok(()),
    }
}
