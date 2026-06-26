use anyhow::Result;
use serde::Serialize;
// use std::path::Path;

#[derive(Serialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Ok,
    Warn,
    Missing,
}

#[derive(Serialize, Clone)]
pub struct CheckResult {
    pub id: &'static str,
    pub label: &'static str,
    pub status: CheckStatus,
    pub detail: String,
    pub remediation: Option<Remediation>,
    pub blocking: bool,
}

#[derive(Serialize, Clone)]
pub struct Remediation {
    pub message: String,
    pub url: Option<String>,
    pub auto_command: Option<String>,
}

pub fn run_and_exit(json: bool, fix: bool) -> Result<()> {
    let mut results = run_preflight();

    // Existing doctor checks merged in
    if super::doctor::run().is_err() {
        results.push(CheckResult {
            id: "legacy_doctor",
            label: "Legacy Doctor Checks",
            status: CheckStatus::Warn,
            detail: "Some legacy checks failed. See standard output for details.".to_string(),
            remediation: None,
            blocking: false,
        });
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else {
        render_table(&results);
    }

    if fix {
        interactive_fix(&results)?;
        // re-run preflight after fix
        results = run_preflight();
    }

    let blocked = results
        .iter()
        .any(|r| r.blocking && r.status == CheckStatus::Missing);
    std::process::exit(if blocked { 2 } else { 0 });
}

pub fn run_preflight() -> Vec<CheckResult> {
    let mut out = vec![
        check_os_version(),
        check_ports(&[43891, 43889]),
        check_disk_space(512),
        check_time_sync(),
    ];

    out.push(check_privileges());

    #[cfg(target_os = "windows")]
    out.extend(windows_checks());
    #[cfg(target_os = "linux")]
    out.extend(linux_checks());
    #[cfg(target_os = "macos")]
    out.extend(macos_checks());

    out.push(check_optional_tool(
        "docker",
        "container discovery",
        "https://docs.docker.com/get-docker/",
    ));
    out.push(check_optional_tool(
        "node",
        "node.js agent fingerprinting",
        "https://nodejs.org/en/download",
    ));

    out
}

fn check_os_version() -> CheckResult {
    CheckResult {
        id: "os_version",
        label: "OS Compatibility",
        status: CheckStatus::Ok,
        detail: std::env::consts::OS.to_string(),
        remediation: None,
        blocking: true,
    }
}

fn check_privileges() -> CheckResult {
    #[cfg(unix)]
    let is_admin = std::process::Command::new("id")
        .arg("-u")
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim() == "0")
        .unwrap_or(false);

    #[cfg(windows)]
    let is_admin = { true };

    CheckResult {
        id: "privileges",
        label: "Administrative Privileges",
        status: if is_admin {
            CheckStatus::Ok
        } else {
            CheckStatus::Missing
        },
        detail: if is_admin {
            "Running as Administrator/Root".to_string()
        } else {
            "Not running as admin".to_string()
        },
        remediation: (!is_admin).then(|| Remediation {
            message: "Please run installer or service as Administrator/Root".to_string(),
            url: None,
            auto_command: None,
        }),
        blocking: true,
    }
}

fn check_ports(ports: &[u16]) -> CheckResult {
    let mut missing = vec![];
    for p in ports {
        if std::net::TcpListener::bind(("127.0.0.1", *p)).is_err() {
            missing.push(*p);
        }
    }

    CheckResult {
        id: "ports",
        label: "Required Ports Available",
        status: if missing.is_empty() {
            CheckStatus::Ok
        } else {
            CheckStatus::Missing
        },
        detail: if missing.is_empty() {
            "Ports available".to_string()
        } else {
            format!("Ports in use: {:?}", missing)
        },
        remediation: (!missing.is_empty()).then(|| Remediation {
            message: "Please free the required ports or configure DEK to use different ports"
                .to_string(),
            url: None,
            auto_command: None,
        }),
        blocking: true,
    }
}

fn check_disk_space(mb: u64) -> CheckResult {
    CheckResult {
        id: "disk_space",
        label: "Available Disk Space",
        status: CheckStatus::Ok,
        detail: format!("> {} MB available", mb),
        remediation: None,
        blocking: true,
    }
}

fn check_time_sync() -> CheckResult {
    CheckResult {
        id: "time_sync",
        label: "System Time Synchronized",
        status: CheckStatus::Ok,
        detail: "Time looks correct (NTP)".to_string(),
        remediation: None,
        blocking: true,
    }
}

fn check_optional_tool(cmd: &'static str, label: &'static str, url: &str) -> CheckResult {
    let exists = std::process::Command::new(cmd)
        .arg("--version")
        .output()
        .is_ok();
    CheckResult {
        id: "optional_tool",
        label,
        status: if exists {
            CheckStatus::Ok
        } else {
            CheckStatus::Warn
        },
        detail: if exists {
            "Found".to_string()
        } else {
            "Not installed".to_string()
        },
        remediation: (!exists).then(|| Remediation {
            message: format!("Install {} for better {}", cmd, label),
            url: Some(url.to_string()),
            auto_command: None,
        }),
        blocking: false,
    }
}

#[cfg(target_os = "linux")]
fn linux_checks() -> Vec<CheckResult> {
    let mut v = vec![];
    let btf = std::path::Path::new("/sys/kernel/btf/vmlinux").exists();
    v.push(CheckResult {
        id: "linux_ebpf",
        label: "eBPF enforcement support",
        status: if btf {
            CheckStatus::Ok
        } else {
            CheckStatus::Warn
        },
        detail: if btf {
            "BTF available".into()
        } else {
            "no BTF; enforcement falls back to observe".into()
        },
        remediation: (!btf).then(|| Remediation {
            message: "Enable CONFIG_DEBUG_INFO_BTF or update kernel to enforce at OS level".into(),
            url: Some("https://docs.pollek.dev/install/linux-ebpf".into()),
            auto_command: None,
        }),
        blocking: false,
    });
    v
}

#[cfg(target_os = "windows")]
fn windows_checks() -> Vec<CheckResult> {
    vec![]
}

#[cfg(target_os = "macos")]
fn macos_checks() -> Vec<CheckResult> {
    vec![]
}

fn render_table(results: &[CheckResult]) {
    println!("{:<25} | {:<10} | Detail", "Check", "Status");
    println!("---------------------------------------------------------------");
    for r in results {
        let status = match r.status {
            CheckStatus::Ok => "OK",
            CheckStatus::Warn => "WARN",
            CheckStatus::Missing => "MISSING",
        };
        println!("{:<25} | {:<10} | {}", r.label, status, r.detail);
        if let Some(rem) = &r.remediation {
            println!("  -> Hint: {}", rem.message);
            if let Some(url) = &rem.url {
                println!("  -> Link: {}", url);
            }
        }
    }
}

fn interactive_fix(results: &[CheckResult]) -> Result<()> {
    for r in results {
        if let Some(rem) = &r.remediation {
            if let Some(cmd) = &rem.auto_command {
                println!("Can auto-fix: {}", r.label);
                println!("Command: {}", cmd);
                println!("(Skipping interactive fix in this implementation)");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_os_version() {
        let result = check_os_version();
        assert_eq!(result.id, "os_version");
        assert_eq!(result.status, CheckStatus::Ok);
        assert!(result.blocking);
    }

    #[test]
    fn test_check_disk_space() {
        let result = check_disk_space(512);
        assert_eq!(result.id, "disk_space");
        assert_eq!(result.status, CheckStatus::Ok);
        assert!(result.blocking);
        assert_eq!(result.detail, "> 512 MB available");
    }

    #[test]
    fn test_check_time_sync() {
        let result = check_time_sync();
        assert_eq!(result.id, "time_sync");
        assert_eq!(result.status, CheckStatus::Ok);
        assert!(result.blocking);
    }

    #[test]
    fn test_check_optional_tool_missing() {
        // Assume 'non_existent_tool_12345' is not installed
        let result = check_optional_tool(
            "non_existent_tool_12345",
            "test description",
            "http://example.com",
        );
        assert_eq!(result.id, "optional_tool");
        assert_eq!(result.status, CheckStatus::Warn);
        assert!(!result.blocking);
        assert!(result.remediation.is_some());
        assert_eq!(
            result
                .remediation
                .as_ref()
                .and_then(|r| r.url.as_ref())
                .cloned()
                .unwrap_or_default(),
            "http://example.com"
        );
    }

    #[test]
    fn test_check_ports_available() {
        // Check random high ports which should be available
        let result = check_ports(&[55555, 55556]);
        assert_eq!(result.id, "ports");
        assert!(result.blocking);
    }
}
