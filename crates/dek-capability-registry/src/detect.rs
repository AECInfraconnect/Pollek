use crate::{CapabilityStatus, PepCapability};
use dek_domain_schema::control_level::ControlLevel;
use dek_domain_schema::deployment_session::LocalizedText;

pub fn detect_pep_capabilities() -> Vec<PepCapability> {
    let mut caps = vec![
        PepCapability {
            r#type: "mcp-stdio".into(),
            transports: vec!["stdio".into()],
            control_level: ControlLevel::Enforce,
            status: CapabilityStatus::Ready,
            status_reason: None,
        },
        PepCapability {
            r#type: "mcp-http".into(),
            transports: vec!["http".into()],
            control_level: ControlLevel::Enforce,
            status: CapabilityStatus::Ready,
            status_reason: None,
        },
    ];

    #[cfg(target_os = "linux")]
    caps.push(detect_ebpf());

    #[cfg(target_os = "windows")]
    caps.push(detect_windows_wfp());

    #[cfg(target_os = "macos")]
    caps.push(detect_macos_nefilter());

    caps
}

#[cfg(any(target_os = "windows", target_os = "macos", test))]
#[derive(Debug, Clone)]
struct NativeNetworkProbe {
    component_present: bool,
    permission_ready: bool,
    warm_check_passed: bool,
    missing_reason: Option<String>,
}

fn localized(en: impl Into<String>) -> LocalizedText {
    let en = en.into();
    LocalizedText { th: en.clone(), en }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn env_is(value: &str, expected: &str) -> bool {
    std::env::var(value)
        .map(|v| v.eq_ignore_ascii_case(expected))
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
fn command_stdout_contains(program: &str, args: &[&str], needle: &str) -> bool {
    std::process::Command::new(program)
        .args(args)
        .output()
        .ok()
        .map(|out| String::from_utf8_lossy(&out.stdout).contains(needle))
        .unwrap_or(false)
}

#[cfg(any(target_os = "windows", target_os = "macos", test))]
fn native_network_capability(
    r#type: &str,
    transport: &str,
    probe: NativeNetworkProbe,
) -> PepCapability {
    let (status, control_level, reason) = if !probe.component_present {
        (
            CapabilityStatus::MissingDriver,
            ControlLevel::Observe,
            probe.missing_reason.or_else(|| {
                Some("Native network enforcement component is not installed or active".into())
            }),
        )
    } else if !probe.permission_ready {
        (
            CapabilityStatus::MissingPermission,
            ControlLevel::Observe,
            Some("Administrator approval is required before native network enforcement".into()),
        )
    } else if probe.warm_check_passed {
        (CapabilityStatus::Ready, ControlLevel::Enforce, None)
    } else {
        (
            CapabilityStatus::InstalledInactive,
            ControlLevel::Observe,
            Some(
                "Native network component is present, but the active enforce warm-check has not passed"
                    .into(),
            ),
        )
    };

    PepCapability {
        r#type: r#type.into(),
        transports: vec![transport.into()],
        control_level,
        status,
        status_reason: reason.map(localized),
    }
}

#[cfg(target_os = "linux")]
fn detect_ebpf() -> PepCapability {
    let has_bpf = std::path::Path::new("/sys/fs/bpf").exists();
    let has_root = std::env::var("USER").unwrap_or_default() == "root";

    let (status, reason, level) = if !has_bpf {
        (
            CapabilityStatus::MissingBinary,
            Some(localized("BPF filesystem is not mounted")),
            ControlLevel::Observe,
        )
    } else if !has_root {
        (
            CapabilityStatus::MissingPermission,
            Some(localized(
                "Root or CAP_BPF/CAP_NET_ADMIN privileges are required for eBPF",
            )),
            ControlLevel::Observe,
        )
    } else {
        (CapabilityStatus::Ready, None, ControlLevel::Enforce)
    };

    PepCapability {
        r#type: "linux-ebpf".into(),
        transports: vec!["ebpf".into()],
        control_level: level,
        status,
        status_reason: reason,
    }
}

#[cfg(target_os = "windows")]
fn detect_windows_wfp() -> PepCapability {
    let bfe_running = command_stdout_contains("sc", &["query", "BFE"], "RUNNING");
    let service_running = env_is("POLLEK_WFP_SERVICE_READY", "1")
        || ["PollekDEK", "PollekDEKCore", "PollekWFP"]
            .iter()
            .any(|service| command_stdout_contains("sc", &["query", service], "RUNNING"));
    let driver_present = env_is("POLLEK_WFP_DRIVER_PRESENT", "1") || service_running;
    let warm_check_passed = env_is("POLLEK_WFP_WARM_CHECK", "passed")
        || (service_running && env_is("POLLEK_WFP_SYNTHETIC_DENY_PASSED", "1"));

    native_network_capability(
        "windows-wfp",
        "wfp",
        NativeNetworkProbe {
            component_present: bfe_running && driver_present,
            permission_ready: service_running || env_is("POLLEK_WFP_ADMIN_APPROVED", "1"),
            warm_check_passed,
            missing_reason: if !bfe_running {
                Some("BFE service is not running".into())
            } else if !driver_present {
                Some("Pollek WFP service or driver is not active".into())
            } else {
                None
            },
        },
    )
}

#[cfg(target_os = "macos")]
fn detect_macos_nefilter() -> PepCapability {
    let extension_list = std::process::Command::new("systemextensionsctl")
        .arg("list")
        .output()
        .ok()
        .map(|out| String::from_utf8_lossy(&out.stdout).to_string())
        .unwrap_or_default();
    let extension_present = env_is("POLLEK_NEFILTER_EXTENSION_PRESENT", "1")
        || extension_list.contains("com.aecinfraconnect.pollek.dek.nefilter")
        || extension_list.contains("com.pollek.nefilter");
    let approved = env_is("POLLEK_NEFILTER_APPROVED", "1")
        || extension_list.contains("[activated enabled]")
        || extension_list.contains("activated enabled");
    let socket_ready = env_is("POLLEK_NEFILTER_SOCKET_READY", "1")
        || std::path::Path::new("/var/run/pollek/nefilter.sock").exists();
    let warm_check_passed = env_is("POLLEK_NEFILTER_WARM_CHECK", "passed")
        || (approved && socket_ready && env_is("POLLEK_NEFILTER_SYNTHETIC_DENY_PASSED", "1"));

    native_network_capability(
        "macos-nefilter",
        "nefilter",
        NativeNetworkProbe {
            component_present: extension_present,
            permission_ready: approved && socket_ready,
            warm_check_passed,
            missing_reason: (!extension_present)
                .then(|| "Network Extension is not installed or active".into()),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_probe_observe_until_warm_check_passes() {
        let cap = native_network_capability(
            "windows-wfp",
            "wfp",
            NativeNetworkProbe {
                component_present: true,
                permission_ready: true,
                warm_check_passed: false,
                missing_reason: None,
            },
        );
        assert_eq!(cap.control_level, ControlLevel::Observe);
        assert_eq!(cap.status, CapabilityStatus::InstalledInactive);
    }

    #[test]
    fn native_probe_enforces_after_warm_check() {
        let cap = native_network_capability(
            "macos-nefilter",
            "nefilter",
            NativeNetworkProbe {
                component_present: true,
                permission_ready: true,
                warm_check_passed: true,
                missing_reason: None,
            },
        );
        assert_eq!(cap.control_level, ControlLevel::Enforce);
        assert_eq!(cap.status, CapabilityStatus::Ready);
    }
}
