// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use anyhow::Result;
use dek_domain_schema::CompiledNetworkRules;

pub mod planner;
pub mod router;

/// Core interface for OS-level enforcement mechanisms (WFP on Windows, NetworkExtension on macOS, eBPF on Linux).
pub trait NetworkEnforcer: Send + Sync {
    /// Starts the enforcement module (e.g. opens handles, registers providers).
    fn start(&mut self) -> Result<()>;

    /// Stops the enforcement module.
    fn stop(&mut self) -> Result<()>;

    /// Applies compiled network rules.
    fn apply_rules(&self, rules: &CompiledNetworkRules) -> Result<()>;

    /// Clears all currently applied network rules.
    fn clear_rules(&self) -> Result<()>;
}

pub mod feasibility;
pub mod warm_check;
