//! Host capability snapshot construction: runtime/demo-profile resolution,
//! OS + elevation detection, per-domain control method readiness, and the
//! v2 -> legacy snapshot projection.

use super::*;

#[derive(Deserialize)]
pub(super) struct ModeQuery {
    pub(super) mode: Option<String>,
    pub(super) demo_os: Option<String>,
    pub(super) demo_profile: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DemoProfile {
    os_family: String,
    profile: String,
}

pub(super) fn demo_profiles_enabled() -> bool {
    std::env::var("POLLEK_ENABLE_DEMO_PROFILES")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

pub(super) fn demo_profile_from_parts(
    demo_os: Option<&str>,
    demo_profile: Option<&str>,
) -> Option<DemoProfile> {
    if !demo_profiles_enabled() {
        return None;
    }

    let os_family = match demo_os?.to_ascii_lowercase().as_str() {
        "windows" | "win" => "windows",
        "macos" | "darwin" | "mac" => "macos",
        "linux" => "linux",
        _ => return None,
    };
    let profile = match demo_profile
        .unwrap_or("ready")
        .to_ascii_lowercase()
        .as_str()
    {
        "ready" | "enforce" => "ready",
        "observe" | "observe_only" => "observe_only",
        "needs_setup" | "setup" => "needs_setup",
        _ => return None,
    };

    Some(DemoProfile {
        os_family: os_family.into(),
        profile: profile.into(),
    })
}

pub(super) fn demo_profile_from_query(query: &ModeQuery) -> Option<DemoProfile> {
    demo_profile_from_parts(query.demo_os.as_deref(), query.demo_profile.as_deref())
}

impl DemoProfile {
    pub(super) fn device_id(&self) -> String {
        format!("demo_{}_{}", self.os_family, self.profile)
    }

    fn readiness(&self) -> MethodReadiness {
        match self.profile.as_str() {
            "ready" => MethodReadiness::Available,
            "observe_only" => MethodReadiness::Degraded,
            "needs_setup" => MethodReadiness::NeedsInstall,
            _ => MethodReadiness::NeedsConfiguration,
        }
    }

    fn warm_check(&self) -> WarmCheckStatus {
        match self.profile.as_str() {
            "ready" => WarmCheckStatus::Passed,
            "observe_only" => WarmCheckStatus::Failed,
            "needs_setup" => WarmCheckStatus::NotRun,
            _ => WarmCheckStatus::NotRun,
        }
    }
}

pub(super) fn parse_mode(value: Option<&str>) -> RuntimeMode {
    match value.unwrap_or("desktop_simple") {
        "desktop_advanced" => RuntimeMode::DesktopAdvanced,
        "enterprise" | "enterprise_server" => RuntimeMode::EnterpriseServer,
        "sovereign" => RuntimeMode::Sovereign,
        "air_gap" | "sovereign_airgap" => RuntimeMode::AirGap,
        _ => RuntimeMode::DesktopSimple,
    }
}

pub(super) fn local_device_id() -> String {
    if let Ok(id) = std::env::var("POLLEK_DEVICE_ID") {
        let trimmed = id.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    let host = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "local-device".to_string());
    let mut hasher = sha2::Sha256::new();
    hasher.update(host.as_bytes());
    let digest = hasher.finalize();
    format!("dev_{}", hex::encode(&digest[..8]))
}

pub(super) fn is_elevated() -> bool {
    if std::env::var("POLLEK_TEST_ELEVATED").ok().as_deref() == Some("1") {
        return true;
    }

    #[cfg(windows)]
    {
        false
    }
    #[cfg(not(windows))]
    {
        std::env::var("USER").ok().as_deref() == Some("root")
    }
}

pub(super) fn os_info_v2_for(demo: Option<&DemoProfile>) -> OsInfoV2 {
    let family = match std::env::consts::OS {
        "macos" => "macos",
        "windows" => "windows",
        "linux" => "linux",
        other => other,
    }
    .to_string();
    let family = demo
        .map(|profile| profile.os_family.clone())
        .unwrap_or(family);
    OsInfoV2 {
        family,
        version: demo
            .map(|profile| format!("demo-{}", profile.profile))
            .unwrap_or_else(|| {
                std::env::var("POLLEK_OS_VERSION").unwrap_or_else(|_| "unknown".into())
            }),
        arch: std::env::consts::ARCH.to_string(),
        is_server: std::env::var("POLLEK_SERVER_MODE").ok().as_deref() == Some("1"),
        elevated: demo
            .map(|profile| profile.profile == "ready")
            .unwrap_or_else(is_elevated),
    }
}

pub(super) fn setup_action(
    action_id: &str,
    title_en: &str,
    title_th: &str,
    detail_en: &str,
    detail_th: &str,
    requires_admin: bool,
) -> SetupAction {
    SetupAction {
        action_id: action_id.to_string(),
        title_en: title_en.to_string(),
        title_th: title_th.to_string(),
        detail_en: detail_en.to_string(),
        detail_th: detail_th.to_string(),
        requires_admin,
        requires_restart: false,
        estimated_minutes: if requires_admin { 3 } else { 1 },
        docs_path: Some(format!("/docs/setup/{action_id}")),
        safe_to_skip: true,
    }
}

pub(super) fn method(
    method_id: &str,
    display_names: (&str, &str),
    domains: Vec<ControlDomainV2>,
    max_level: ControlLevelV2,
    status: MethodReadiness,
    maturity: MethodMaturity,
    setup_action_ids: Vec<String>,
) -> ControlMethodCapabilityV2 {
    let install_state = match status {
        MethodReadiness::Available | MethodReadiness::Degraded | MethodReadiness::SimulatorOnly => {
            InstallState::Installed
        }
        MethodReadiness::NeedsInstall => InstallState::NotInstalled,
        MethodReadiness::NeedsConfiguration => InstallState::InstalledButDisabled,
        MethodReadiness::NeedsPermission => InstallState::Installed,
        MethodReadiness::Unsupported => InstallState::ExternalRequired,
        MethodReadiness::Failed => InstallState::Unknown,
    };

    ControlMethodCapabilityV2 {
        method_id: method_id.to_string(),
        display_name_en: display_names.0.to_string(),
        display_name_th: display_names.1.to_string(),
        domains,
        max_level,
        status,
        maturity,
        install_state,
        warm_check: Some(WarmCheckStatus::NotRun),
        setup_action_ids,
        limitations_en: Vec::new(),
        limitations_th: Vec::new(),
    }
}

pub(super) fn build_capability_snapshot_v2(
    tenant_id: &str,
    device_id: &str,
    mode: RuntimeMode,
) -> LocalCapabilitySnapshotV2 {
    build_capability_snapshot_v2_for(tenant_id, device_id, mode, None)
}

pub(super) fn build_capability_snapshot_v2_for(
    tenant_id: &str,
    device_id: &str,
    mode: RuntimeMode,
    demo: Option<&DemoProfile>,
) -> LocalCapabilitySnapshotV2 {
    let os = os_info_v2_for(demo);
    let mut setup_actions = vec![
        setup_action(
            "approve_mcp_config_wrapper",
            "Allow Pollek to wrap agent tool configuration",
            "อนุญาตให้ Pollek ครอบการตั้งค่าเครื่องมือของ Agent",
            "Required before Pollek can enforce MCP tool calls for agents that are not already routed through a Pollek wrapper or proxy.",
            "จำเป็นก่อนที่ Pollek จะบังคับใช้ MCP tool calls สำหรับ Agent ที่ยังไม่ได้วิ่งผ่าน wrapper หรือ proxy ของ Pollek",
            false,
        ),
        setup_action(
            "install_browser_extension",
            "Install Pollek browser extension",
            "ติดตั้ง Pollek browser extension",
            "Required to observe or control browser-based AI sessions.",
            "จำเป็นสำหรับการสังเกตหรือควบคุม AI session บน browser",
            false,
        ),
    ];

    let mut methods = vec![
        method(
            "mcp_stdio_wrapper",
            ("Agent tool control", "การควบคุมเครื่องมือของ Agent"),
            vec![
                ControlDomainV2::McpToolCall,
                ControlDomainV2::PromptContent,
                ControlDomainV2::FileAccess,
                ControlDomainV2::SkillRuntime,
            ],
            ControlLevelV2::Enforce,
            if std::env::var("POLLEK_MCP_STDIO_WRAPPER_READY")
                .ok()
                .as_deref()
                == Some("1")
            {
                MethodReadiness::Available
            } else {
                MethodReadiness::NeedsConfiguration
            },
            MethodMaturity::Beta,
            vec!["approve_mcp_config_wrapper".into()],
        ),
        method(
            "mcp_http_proxy",
            ("Agent HTTP tool proxy", "Proxy เครื่องมือ HTTP ของ Agent"),
            vec![
                ControlDomainV2::McpToolCall,
                ControlDomainV2::PromptContent,
                ControlDomainV2::TokenCost,
            ],
            ControlLevelV2::Enforce,
            if std::env::var("POLLEK_MCP_PROXY_READY").ok().as_deref() == Some("1") {
                MethodReadiness::Available
            } else {
                MethodReadiness::NeedsConfiguration
            },
            MethodMaturity::Beta,
            vec!["approve_mcp_config_wrapper".into()],
        ),
        method(
            "wasm_policy_evaluator",
            ("WASM policy evaluator", "ตัวประเมินนโยบาย WASM"),
            vec![
                ControlDomainV2::PromptContent,
                ControlDomainV2::TokenCost,
                ControlDomainV2::SkillRuntime,
            ],
            ControlLevelV2::Warn,
            MethodReadiness::Available,
            MethodMaturity::Production,
            vec![],
        ),
        method(
            "egress_simulator",
            ("Egress simulator", "ตัวจำลอง egress"),
            vec![ControlDomainV2::NetworkEgress, ControlDomainV2::Dns],
            ControlLevelV2::Observe,
            MethodReadiness::SimulatorOnly,
            MethodMaturity::Simulator,
            vec![],
        ),
    ];

    match os.family.as_str() {
        "windows" => {
            setup_actions.push(setup_action(
                "install_windows_wfp_service",
                "Install Windows network control",
                "ติดตั้งตัวควบคุมเครือข่าย Windows",
                "Required before Pollek can block real network egress with Windows Filtering Platform.",
                "จำเป็นก่อนที่ Pollek จะบล็อก network egress จริงด้วย Windows Filtering Platform ได้",
                true,
            ));
            let installed =
                std::path::Path::new("C:\\Program Files\\Pollek\\pollek-wfp-service.exe").exists();
            let status = if !installed {
                MethodReadiness::NeedsInstall
            } else if !os.elevated {
                MethodReadiness::NeedsPermission
            } else if std::env::var("POLLEK_WFP_WARM_CHECK").ok().as_deref() == Some("passed") {
                MethodReadiness::Available
            } else {
                MethodReadiness::NeedsConfiguration
            };
            methods.push(method(
                "windows_wfp",
                ("Device-level network control", "การควบคุมเครือข่ายระดับเครื่อง"),
                vec![ControlDomainV2::NetworkEgress, ControlDomainV2::Dns],
                ControlLevelV2::Enforce,
                status,
                MethodMaturity::Preview,
                vec!["install_windows_wfp_service".into()],
            ));
            methods.push(method(
                "windows_etw_process_observer",
                (
                    "Windows process activity observer",
                    "ตัวสังเกตกิจกรรม process บน Windows",
                ),
                vec![ControlDomainV2::ProcessLaunch],
                ControlLevelV2::Observe,
                MethodReadiness::NeedsConfiguration,
                MethodMaturity::Preview,
                vec![],
            ));
        }
        "macos" => {
            setup_actions.push(setup_action(
                "approve_macos_network_extension",
                "Approve macOS Network Extension",
                "อนุมัติ macOS Network Extension",
                "Required before Pollek can observe or block real network traffic on macOS.",
                "จำเป็นก่อนที่ Pollek จะสังเกตหรือบล็อกทราฟฟิกจริงบน macOS ได้",
                true,
            ));
            setup_actions.push(setup_action(
                "approve_macos_endpoint_security",
                "Approve macOS Endpoint Security extension",
                "อนุมัติ macOS Endpoint Security extension",
                "Required before Pollek can observe sensitive process and file activity on macOS.",
                "จำเป็นก่อนที่ Pollek จะสังเกตกิจกรรม process และไฟล์ที่อ่อนไหวบน macOS ได้",
                true,
            ));
            methods.push(method(
                "macos_network_extension",
                ("Device-level network control", "การควบคุมเครือข่ายระดับเครื่อง"),
                vec![ControlDomainV2::NetworkEgress, ControlDomainV2::Dns],
                ControlLevelV2::Enforce,
                MethodReadiness::NeedsPermission,
                MethodMaturity::Preview,
                vec!["approve_macos_network_extension".into()],
            ));
            methods.push(method(
                "macos_endpoint_security",
                (
                    "macOS process and file observer",
                    "ตัวสังเกต process และไฟล์บน macOS",
                ),
                vec![ControlDomainV2::ProcessLaunch, ControlDomainV2::FileAccess],
                ControlLevelV2::Observe,
                MethodReadiness::NeedsPermission,
                MethodMaturity::Preview,
                vec!["approve_macos_endpoint_security".into()],
            ));
        }
        "linux" => {
            setup_actions.push(setup_action(
                "grant_linux_ebpf_permissions",
                "Grant Linux eBPF permissions",
                "ให้สิทธิ์ Linux eBPF",
                "Required before Pollek can load eBPF programs for real network observation or blocking.",
                "จำเป็นก่อนที่ Pollek จะโหลด eBPF program เพื่อสังเกตหรือบล็อกเครือข่ายจริงได้",
                true,
            ));
            setup_actions.push(setup_action(
                "grant_linux_fanotify_permissions",
                "Grant Linux file activity permissions",
                "ให้สิทธิ์ติดตามกิจกรรมไฟล์บน Linux",
                "Required before Pollek can use fanotify permission events for file control.",
                "จำเป็นก่อนที่ Pollek จะใช้ fanotify permission events เพื่อควบคุมไฟล์ได้",
                true,
            ));
            let bpf_fs = std::path::Path::new("/sys/fs/bpf").exists();
            let ebpf_status =
                if std::env::var("POLLEK_EBPF_PROGRAM_LOADED").ok().as_deref() == Some("1") {
                    MethodReadiness::Available
                } else if !bpf_fs {
                    MethodReadiness::NeedsInstall
                } else if !os.elevated {
                    MethodReadiness::NeedsPermission
                } else {
                    MethodReadiness::NeedsConfiguration
                };
            methods.push(method(
                "linux_ebpf",
                ("Device-level network control", "การควบคุมเครือข่ายระดับเครื่อง"),
                vec![ControlDomainV2::NetworkEgress, ControlDomainV2::Dns],
                ControlLevelV2::Enforce,
                ebpf_status,
                MethodMaturity::Preview,
                vec!["grant_linux_ebpf_permissions".into()],
            ));
            methods.push(method(
                "linux_fanotify",
                ("Linux file activity control", "การควบคุมกิจกรรมไฟล์บน Linux"),
                vec![ControlDomainV2::FileAccess],
                ControlLevelV2::Ask,
                if os.elevated {
                    MethodReadiness::Degraded
                } else {
                    MethodReadiness::NeedsPermission
                },
                MethodMaturity::Preview,
                vec!["grant_linux_fanotify_permissions".into()],
            ));
        }
        _ => {}
    }

    let mut observation_sources = vec![
        ObservationSourceCapability {
            source_id: "process_metadata".into(),
            display_name_en: "Process metadata scan".into(),
            display_name_th: "การสแกน metadata ของ process".into(),
            status: MethodReadiness::Available,
            domains: vec![ControlDomainV2::ProcessLaunch],
            privacy_note_en: "Collects process names, redacted paths, and hashes; it does not collect prompt content.".into(),
            privacy_note_th: "เก็บชื่อ process, path ที่ redacted แล้ว และ hash โดยไม่เก็บ prompt content".into(),
            setup_action_ids: vec![],
        },
        ObservationSourceCapability {
            source_id: "mcp_config_scan".into(),
            display_name_en: "MCP configuration scan".into(),
            display_name_th: "การสแกน MCP configuration".into(),
            status: MethodReadiness::Available,
            domains: vec![ControlDomainV2::McpToolCall, ControlDomainV2::SkillRuntime],
            privacy_note_en: "Checks known configuration locations and stores redacted evidence.".into(),
            privacy_note_th: "ตรวจตำแหน่ง configuration ที่รู้จักและเก็บหลักฐานแบบ redacted".into(),
            setup_action_ids: vec![],
        },
        ObservationSourceCapability {
            source_id: "browser_extension".into(),
            display_name_en: "Browser AI observer".into(),
            display_name_th: "ตัวสังเกต AI บน Browser".into(),
            status: MethodReadiness::NeedsInstall,
            domains: vec![ControlDomainV2::BrowserAiSession],
            privacy_note_en: "Requires the browser extension before browser AI sessions can be observed.".into(),
            privacy_note_th: "ต้องติดตั้ง browser extension ก่อนจึงจะสังเกต AI session บน browser ได้".into(),
            setup_action_ids: vec!["install_browser_extension".into()],
        },
        ObservationSourceCapability {
            source_id: "structured_local_agent_logs".into(),
            display_name_en: "Structured local agent logs".into(),
            display_name_th: "บันทึกกิจกรรมของ Agent ในเครื่อง".into(),
            status: MethodReadiness::Available,
            domains: vec![
                ControlDomainV2::FileAccess,
                ControlDomainV2::NetworkEgress,
                ControlDomainV2::McpToolCall,
                ControlDomainV2::PromptContent,
                ControlDomainV2::TokenCost,
            ],
            privacy_note_en: "Reads approved local agent/session logs when present; stores redacted paths, domains, usage fields, and decisions, not raw prompt or file contents by default.".into(),
            privacy_note_th: "อ่าน log/session ในเครื่องที่อนุญาตไว้เมื่อพบ เก็บ path/domain/usage/decision แบบ redacted โดยไม่เก็บ prompt หรือเนื้อหาไฟล์เป็นค่าเริ่มต้น".into(),
            setup_action_ids: vec![],
        },
        ObservationSourceCapability {
            source_id: "wrapper_or_proxy_telemetry".into(),
            display_name_en: "Wrapper or proxy telemetry".into(),
            display_name_th: "Telemetry จาก wrapper หรือ proxy".into(),
            status: if std::env::var("POLLEK_MCP_PROXY_READY").ok().as_deref() == Some("1")
                || std::env::var("POLLEK_MCP_STDIO_WRAPPER_READY").ok().as_deref() == Some("1")
            {
                MethodReadiness::Available
            } else {
                MethodReadiness::NeedsConfiguration
            },
            domains: vec![
                ControlDomainV2::McpToolCall,
                ControlDomainV2::PromptContent,
                ControlDomainV2::TokenCost,
                ControlDomainV2::FileAccess,
                ControlDomainV2::NetworkEgress,
            ],
            privacy_note_en: "Provides exact tool/resource/model usage when an AI app is routed through an approved Pollek wrapper or proxy.".into(),
            privacy_note_th: "ให้ข้อมูล tool/resource/model usage แบบ exact เมื่อ AI app วิ่งผ่าน wrapper หรือ proxy ของ Pollek ที่อนุญาตไว้".into(),
            setup_action_ids: vec!["approve_mcp_config_wrapper".into()],
        },
    ];

    match os.family.as_str() {
        "windows" => {
            setup_actions.push(setup_action(
                "enable_windows_etw_observer",
                "Enable Windows activity observer",
                "เปิดตัวสังเกตกิจกรรม Windows",
                "Required before Pollek can collect deeper Windows process, file, and network metadata through ETW or OS audit sources.",
                "จำเป็นก่อนที่ Pollek จะเก็บ metadata ระดับลึกของ process, file และ network บน Windows ผ่าน ETW หรือ OS audit source ได้",
                true,
            ));
            observation_sources.push(ObservationSourceCapability {
                source_id: "windows_etw_observer".into(),
                display_name_en: "Windows ETW activity observer".into(),
                display_name_th: "ตัวสังเกตกิจกรรม Windows ETW".into(),
                status: if std::env::var("POLLEK_WINDOWS_ETW_OBSERVER_READY").ok().as_deref()
                    == Some("1")
                {
                    MethodReadiness::Available
                } else if os.elevated {
                    MethodReadiness::NeedsConfiguration
                } else {
                    MethodReadiness::NeedsPermission
                },
                domains: vec![
                    ControlDomainV2::ProcessLaunch,
                    ControlDomainV2::FileAccess,
                    ControlDomainV2::NetworkEgress,
                    ControlDomainV2::Dns,
                ],
                privacy_note_en: "Uses Windows event tracing/audit metadata for process, file, DNS, and network signals where enabled; content bodies are not collected.".into(),
                privacy_note_th: "ใช้ metadata จาก Windows event tracing/audit สำหรับ process, file, DNS และ network เมื่อเปิดใช้งาน โดยไม่เก็บเนื้อหา".into(),
                setup_action_ids: vec!["enable_windows_etw_observer".into()],
            });
            observation_sources.push(ObservationSourceCapability {
                source_id: "windows_directory_changes".into(),
                display_name_en: "Windows folder change watcher".into(),
                display_name_th: "ตัวติดตามการเปลี่ยนแปลงโฟลเดอร์ Windows".into(),
                status: MethodReadiness::Degraded,
                domains: vec![ControlDomainV2::FileAccess],
                privacy_note_en: "Can watch selected folders for created, changed, renamed, or deleted files. It does not prove which process caused the change without another signal.".into(),
                privacy_note_th: "ติดตามโฟลเดอร์ที่เลือกว่ามีไฟล์ถูกสร้าง แก้ไข เปลี่ยนชื่อ หรือลบ แต่ต้องใช้สัญญาณอื่นประกอบเพื่อยืนยัน process".into(),
                setup_action_ids: vec![],
            });
        }
        "macos" => {
            observation_sources.push(ObservationSourceCapability {
                source_id: "macos_endpoint_security".into(),
                display_name_en: "macOS Endpoint Security observer".into(),
                display_name_th: "ตัวสังเกต macOS Endpoint Security".into(),
                status: MethodReadiness::NeedsPermission,
                domains: vec![ControlDomainV2::ProcessLaunch, ControlDomainV2::FileAccess],
                privacy_note_en: "Requires Apple Endpoint Security approval before Pollek can observe sensitive process and file events.".into(),
                privacy_note_th: "ต้องอนุมัติ Apple Endpoint Security ก่อน Pollek จึงจะสังเกต process และ file event ที่อ่อนไหวได้".into(),
                setup_action_ids: vec!["approve_macos_endpoint_security".into()],
            });
            observation_sources.push(ObservationSourceCapability {
                source_id: "macos_fsevents".into(),
                display_name_en: "macOS file-system event watcher".into(),
                display_name_th: "ตัวติดตาม file-system event บน macOS".into(),
                status: MethodReadiness::Degraded,
                domains: vec![ControlDomainV2::FileAccess],
                privacy_note_en: "Can watch folder-tree changes where allowed. It is useful context but not full per-process proof by itself.".into(),
                privacy_note_th: "ติดตามการเปลี่ยนแปลงของ folder tree ที่อนุญาตได้ เป็นบริบทที่มีประโยชน์แต่ยังไม่ใช่หลักฐาน per-process แบบครบถ้วน".into(),
                setup_action_ids: vec![],
            });
            observation_sources.push(ObservationSourceCapability {
                source_id: "macos_network_extension".into(),
                display_name_en: "macOS Network Extension observer".into(),
                display_name_th: "ตัวสังเกต macOS Network Extension".into(),
                status: MethodReadiness::NeedsPermission,
                domains: vec![ControlDomainV2::NetworkEgress, ControlDomainV2::Dns],
                privacy_note_en: "Requires Network Extension approval before device-level network observation or blocking can be real.".into(),
                privacy_note_th: "ต้องอนุมัติ Network Extension ก่อนจึงจะสังเกตหรือบล็อก network ระดับเครื่องได้จริง".into(),
                setup_action_ids: vec!["approve_macos_network_extension".into()],
            });
        }
        "linux" => {
            observation_sources.push(ObservationSourceCapability {
                source_id: "linux_fanotify".into(),
                display_name_en: "Linux fanotify file observer".into(),
                display_name_th: "ตัวสังเกตไฟล์ Linux fanotify".into(),
                status: if os.elevated {
                    MethodReadiness::Degraded
                } else {
                    MethodReadiness::NeedsPermission
                },
                domains: vec![ControlDomainV2::FileAccess],
                privacy_note_en: "Can observe file events and permission decisions when granted required privileges. Exact path/process depth depends on kernel and mount configuration.".into(),
                privacy_note_th: "สังเกต file event และ permission decision ได้เมื่อได้สิทธิ์ที่จำเป็น ความละเอียดของ path/process ขึ้นกับ kernel และ mount configuration".into(),
                setup_action_ids: vec!["grant_linux_fanotify_permissions".into()],
            });
            observation_sources.push(ObservationSourceCapability {
                source_id: "linux_inotify_path_watcher".into(),
                display_name_en: "Linux folder change watcher".into(),
                display_name_th: "ตัวติดตามการเปลี่ยนแปลงโฟลเดอร์ Linux".into(),
                status: MethodReadiness::Degraded,
                domains: vec![ControlDomainV2::FileAccess],
                privacy_note_en: "Can watch selected directories for changes without content capture, but it does not prove the responsible process by itself.".into(),
                privacy_note_th: "ติดตาม directory ที่เลือกได้โดยไม่เก็บเนื้อหา แต่ยังไม่ยืนยัน process ที่ทำให้เกิด event ได้โดยลำพัง".into(),
                setup_action_ids: vec![],
            });
            observation_sources.push(ObservationSourceCapability {
                source_id: "linux_ebpf_observer".into(),
                display_name_en: "Linux eBPF network observer".into(),
                display_name_th: "ตัวสังเกต network Linux eBPF".into(),
                status: if std::env::var("POLLEK_EBPF_PROGRAM_LOADED").ok().as_deref() == Some("1")
                {
                    MethodReadiness::Available
                } else if os.elevated {
                    MethodReadiness::NeedsConfiguration
                } else {
                    MethodReadiness::NeedsPermission
                },
                domains: vec![ControlDomainV2::NetworkEgress, ControlDomainV2::Dns],
                privacy_note_en: "Can provide deeper network metadata when eBPF programs are loaded with required privileges. HTTPS contents are not collected by default.".into(),
                privacy_note_th: "ให้ network metadata ระดับลึกขึ้นเมื่อโหลด eBPF program ด้วยสิทธิ์ที่จำเป็น โดยไม่เก็บเนื้อหา HTTPS เป็นค่าเริ่มต้น".into(),
                setup_action_ids: vec!["grant_linux_ebpf_permissions".into()],
            });
        }
        _ => {}
    }

    if let Some(profile) = demo {
        apply_demo_profile(profile, &mut methods, &mut observation_sources);
    }

    LocalCapabilitySnapshotV2 {
        schema_version: "local-capability-snapshot.v2".into(),
        tenant_id: tenant_id.to_string(),
        device_id: device_id.to_string(),
        os,
        mode,
        generated_at: chrono::Utc::now(),
        control_methods: methods,
        observation_sources,
        setup_actions,
        contract: ContractCompatibilityStatus {
            local_contract_version: "2026.06.26".into(),
            compatible_cloud_contracts: vec![">=2026.06.01 <2026.09.00".into()],
            status: "compatible".into(),
            reason_code: demo.map(|_| "demo_fixture".into()),
        },
    }
}

pub(super) fn apply_demo_profile(
    profile: &DemoProfile,
    methods: &mut [ControlMethodCapabilityV2],
    observation_sources: &mut [ObservationSourceCapability],
) {
    let readiness = profile.readiness();
    let warm_check = profile.warm_check();
    let demo_note = format!(
        "Demo fixture for {} {}. Not evidence of the current host capability.",
        profile.os_family, profile.profile
    );

    for method in methods {
        if matches!(
            method.method_id.as_str(),
            "mcp_stdio_wrapper"
                | "mcp_http_proxy"
                | "linux_ebpf"
                | "linux_fanotify"
                | "windows_wfp"
                | "windows_etw_process_observer"
                | "macos_network_extension"
                | "macos_endpoint_security"
        ) {
            method.status = readiness.clone();
            method.warm_check = Some(warm_check.clone());
            if profile.profile == "ready" && method.maturity == MethodMaturity::Preview {
                method.maturity = MethodMaturity::Beta;
            }
            method.limitations_en.push(demo_note.clone());
            method.limitations_th.push(demo_note.clone());
        }
    }

    for source in observation_sources {
        if matches!(
            source.source_id.as_str(),
            "browser_extension"
                | "wrapper_or_proxy_telemetry"
                | "windows_etw_observer"
                | "windows_directory_changes"
                | "macos_endpoint_security"
                | "macos_fsevents"
                | "macos_network_extension"
                | "linux_fanotify"
                | "linux_inotify_path_watcher"
                | "linux_ebpf_observer"
        ) {
            source.status = readiness.clone();
            source.privacy_note_en = format!(
                "{} Observation source readiness is simulated for this demo profile.",
                demo_note
            );
            source.privacy_note_th = source.privacy_note_en.clone();
        }
    }
}

pub(super) fn legacy_status(status: &MethodReadiness) -> MethodStatus {
    match status {
        MethodReadiness::Available | MethodReadiness::Degraded | MethodReadiness::SimulatorOnly => {
            MethodStatus::Available
        }
        MethodReadiness::NeedsPermission => MethodStatus::NeedsPermission,
        MethodReadiness::NeedsInstall | MethodReadiness::NeedsConfiguration => {
            MethodStatus::NeedsInstall
        }
        MethodReadiness::Unsupported | MethodReadiness::Failed => MethodStatus::Unsupported,
    }
}

pub(super) fn legacy_domain(domain: &ControlDomainV2) -> Option<ControlDomain> {
    match domain {
        ControlDomainV2::McpToolCall | ControlDomainV2::PromptContent => {
            Some(ControlDomain::McpTool)
        }
        ControlDomainV2::NetworkEgress => Some(ControlDomain::Network),
        ControlDomainV2::Dns => Some(ControlDomain::Dns),
        ControlDomainV2::FileAccess => Some(ControlDomain::FileSystem),
        ControlDomainV2::ProcessLaunch => Some(ControlDomain::Process),
        _ => None,
    }
}

pub(super) fn legacy_level(level: &ControlLevelV2) -> ControlLevel {
    match level {
        ControlLevelV2::Observe => ControlLevel::Observe,
        ControlLevelV2::Warn => ControlLevel::Warn,
        ControlLevelV2::Ask => ControlLevel::Ask,
        ControlLevelV2::Enforce | ControlLevelV2::StrictDeny => ControlLevel::Enforce,
    }
}

pub(super) fn legacy_snapshot_from_v2(
    snapshot: &LocalCapabilitySnapshotV2,
) -> LocalCapabilitySnapshot {
    let control_methods = snapshot
        .control_methods
        .iter()
        .map(|method| {
            let mut domains = method
                .domains
                .iter()
                .filter_map(legacy_domain)
                .collect::<Vec<_>>();
            domains.sort();
            domains.dedup();
            ControlMethodCap {
                id: method.method_id.clone(),
                domains,
                max_level: legacy_level(&method.max_level),
                status: legacy_status(&method.status),
            }
        })
        .collect();
    LocalCapabilitySnapshot { control_methods }
}
