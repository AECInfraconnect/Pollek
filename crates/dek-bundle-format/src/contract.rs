//! Contract Hub — version negotiation between a DEK/LCP and Pollek Cloud.
//!
//! A fleet of DEKs runs on many machines at *different* versions. Cloud authors
//! one bundle; each DEK must decide whether it can safely activate it. This
//! module is the single, pure place that answers that: given the DEK's
//! self-reported [`DekContract`] and a bundle's [`crate::BundleCompatibility`],
//! it returns a [`CompatibilityVerdict`] with explicit reasons — never a silent
//! yes/no. Cloud uses the same logic to serve each DEK the newest bundle it can
//! run (or tell it to upgrade).

use crate::{BundleCompatibility, OsModulesConfig};
use serde::{Deserialize, Serialize};

/// The running DEK's version, sourced from the compiled binary (SSOT). Because
/// this crate inherits `version.workspace`, this is the Pollek product version,
/// not an individual crate version.
pub fn dek_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// What THIS DEK build can actually run — its self-reported contract.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DekContract {
    /// Product version of the running DEK (e.g. `1.0.0-beta.10`).
    pub dek_version: String,
    /// Contract generation this DEK speaks (e.g. `2026.06.29`).
    pub contract_version: String,
    /// Bundle-envelope api versions understood (e.g. `["v1"]`).
    pub supported_bundle_api_versions: Vec<String>,
    /// PEP types this build can bind right now (user-space PEPs are always
    /// present; OS-module PEPs only when the module is genuinely available).
    pub available_pep_types: Vec<String>,
    /// OS enforcement modules genuinely available. A `*.stub` entry means the
    /// module is compiled-out / not available on this host and does NOT count.
    pub os_modules: OsModulesConfig,
    /// Host platform: `linux` | `windows` | `macos`.
    pub platform: String,
}

impl DekContract {
    /// Real OS modules available on this DEK's platform, with `*.stub`
    /// placeholders filtered out (a stub is not a real capability).
    pub fn real_os_modules(&self) -> Vec<String> {
        let list = match self.platform.as_str() {
            "linux" => &self.os_modules.linux,
            "windows" => &self.os_modules.windows,
            "macos" => &self.os_modules.macos,
            _ => return Vec::new(),
        };
        list.iter()
            .filter(|m| !m.ends_with(".stub"))
            .cloned()
            .collect()
    }
}

/// Outcome of evaluating a bundle against a DEK.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityStatus {
    /// The DEK can activate this bundle now.
    Compatible,
    /// The bundle is newer than this DEK; the DEK must upgrade first.
    NeedsUpgrade,
    /// The DEK is missing a required PEP type or OS module — an upgrade alone
    /// will not necessarily help (capability, not just version).
    Unsupported,
}

/// Explicit verdict with human-readable reasons.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompatibilityVerdict {
    pub status: CompatibilityStatus,
    pub reasons: Vec<String>,
    pub missing_pep_types: Vec<String>,
    pub missing_os_modules: Vec<String>,
    pub dek_version: String,
    pub min_dek_version: String,
}

/// Compare two versions leniently. Returns `Some(Ordering)` when both parse as
/// semver (pre-release tags respected), else `None` (unknown — do not block).
fn version_cmp(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    let pa = semver::Version::parse(a.trim_start_matches('v')).ok()?;
    let pb = semver::Version::parse(b.trim_start_matches('v')).ok()?;
    Some(pa.cmp(&pb))
}

/// Evaluate whether `dek` can activate a bundle with the given `compat`.
///
/// Precedence: a missing capability (PEP type / OS module) is `Unsupported`
/// (fix is not just a version bump); otherwise a too-low version is
/// `NeedsUpgrade`; otherwise `Compatible`. `required_crates` is advisory only —
/// the runtime cannot verify linked crates, so it is surfaced as a reason but
/// never blocks.
pub fn evaluate_compatibility(
    dek: &DekContract,
    compat: &BundleCompatibility,
) -> CompatibilityVerdict {
    let mut reasons = Vec::new();

    // Capability: required PEP types.
    let missing_pep_types: Vec<String> = compat
        .required_pep_types
        .iter()
        .filter(|req| !dek.available_pep_types.iter().any(|a| a == *req))
        .cloned()
        .collect();
    for m in &missing_pep_types {
        reasons.push(format!("missing PEP type: {m}"));
    }

    // Capability: required OS modules (for this platform, stubs excluded).
    let available_modules = dek.real_os_modules();
    let required_modules = match dek.platform.as_str() {
        "linux" => &compat.required_os_modules.linux,
        "windows" => &compat.required_os_modules.windows,
        "macos" => &compat.required_os_modules.macos,
        _ => &compat.required_os_modules.linux,
    };
    let missing_os_modules: Vec<String> = required_modules
        .iter()
        .filter(|req| !available_modules.iter().any(|a| a == *req))
        .cloned()
        .collect();
    for m in &missing_os_modules {
        reasons.push(format!("missing OS module ({}): {m}", dek.platform));
    }

    // Version floor.
    let version_too_low = match version_cmp(&dek.dek_version, &compat.min_dek_version) {
        Some(std::cmp::Ordering::Less) => {
            reasons.push(format!(
                "DEK {} is older than required minimum {}",
                dek.dek_version, compat.min_dek_version
            ));
            true
        }
        Some(_) => false,
        None => {
            reasons.push(format!(
                "could not compare versions ({} vs {}); not blocking",
                dek.dek_version, compat.min_dek_version
            ));
            false
        }
    };

    // Advisory: required crates cannot be verified at runtime.
    if !compat.required_crates.is_empty() {
        reasons.push(format!(
            "bundle declares required crates (advisory): {}",
            compat.required_crates.join(", ")
        ));
    }

    let status = if !missing_pep_types.is_empty() || !missing_os_modules.is_empty() {
        CompatibilityStatus::Unsupported
    } else if version_too_low {
        CompatibilityStatus::NeedsUpgrade
    } else {
        if reasons.is_empty() {
            reasons.push("all requirements satisfied".to_string());
        }
        CompatibilityStatus::Compatible
    };

    CompatibilityVerdict {
        status,
        reasons,
        missing_pep_types,
        missing_os_modules,
        dek_version: dek.dek_version.clone(),
        min_dek_version: compat.min_dek_version.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dek(version: &str) -> DekContract {
        DekContract {
            dek_version: version.to_string(),
            contract_version: "2026.06.29".to_string(),
            supported_bundle_api_versions: vec!["v1".to_string()],
            available_pep_types: vec!["mcp_proxy".to_string(), "http_gateway".to_string()],
            os_modules: OsModulesConfig {
                linux: vec!["ebpfd.stub".to_string()],
                windows: vec![],
                macos: vec![],
            },
            platform: "linux".to_string(),
        }
    }

    fn compat(min: &str) -> BundleCompatibility {
        BundleCompatibility {
            min_dek_version: min.to_string(),
            required_crates: vec![],
            required_pep_types: vec![],
            required_os_modules: OsModulesConfig::default(),
        }
    }

    #[test]
    fn compatible_when_version_and_capabilities_ok() {
        let v = evaluate_compatibility(&dek("1.0.0-beta.10"), &compat("1.0.0-beta.6"));
        assert_eq!(v.status, CompatibilityStatus::Compatible);
    }

    #[test]
    fn needs_upgrade_when_dek_older_than_min() {
        let v = evaluate_compatibility(&dek("1.0.0-beta.3"), &compat("1.0.0-beta.6"));
        assert_eq!(v.status, CompatibilityStatus::NeedsUpgrade);
    }

    #[test]
    fn unsupported_when_pep_type_missing() {
        let mut c = compat("1.0.0-beta.6");
        c.required_pep_types = vec!["linux_ebpf".to_string()];
        let v = evaluate_compatibility(&dek("1.0.0-beta.10"), &c);
        assert_eq!(v.status, CompatibilityStatus::Unsupported);
        assert_eq!(v.missing_pep_types, vec!["linux_ebpf".to_string()]);
    }

    #[test]
    fn stub_os_module_does_not_satisfy_requirement() {
        let mut c = compat("1.0.0-beta.6");
        c.required_os_modules = OsModulesConfig {
            linux: vec!["ebpfd.v1".to_string()],
            windows: vec![],
            macos: vec![],
        };
        // DEK only has ebpfd.stub → requirement unmet.
        let v = evaluate_compatibility(&dek("1.0.0-beta.10"), &c);
        assert_eq!(v.status, CompatibilityStatus::Unsupported);
        assert_eq!(v.missing_os_modules, vec!["ebpfd.v1".to_string()]);
    }

    #[test]
    fn capability_gap_outranks_version_gap() {
        let mut c = compat("2.0.0"); // also too new
        c.required_pep_types = vec!["macos_network_extension".to_string()];
        let v = evaluate_compatibility(&dek("1.0.0-beta.10"), &c);
        // missing capability wins over needs-upgrade
        assert_eq!(v.status, CompatibilityStatus::Unsupported);
    }
}
