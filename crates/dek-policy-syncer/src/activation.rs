// SPDX-License-Identifier: Apache-2.0

use anyhow::{bail, Result};
use dek_bundle_format::{OsModulesConfig, PollenPolicyBundle};

pub fn validate_os_capabilities(
    bundle: &PollenPolicyBundle,
    available_caps: &OsModulesConfig,
) -> Result<()> {
    let req = &bundle.compatibility.required_os_modules;

    // Check Linux requirements
    for cap in &req.linux {
        if !available_caps.linux.contains(cap) {
            bail!(
                "Activation rejected: missing required Linux capability '{}'",
                cap
            );
        }
    }

    // Check Windows requirements
    for cap in &req.windows {
        if !available_caps.windows.contains(cap) {
            bail!(
                "Activation rejected: missing required Windows capability '{}'",
                cap
            );
        }
    }

    // Check macOS requirements
    for cap in &req.macos {
        if !available_caps.macos.contains(cap) {
            bail!(
                "Activation rejected: missing required macOS capability '{}'",
                cap
            );
        }
    }

    Ok(())
}
