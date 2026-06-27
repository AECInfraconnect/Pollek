// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use axum::{
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::{error::ApiResult, state::AppState};

#[derive(Serialize)]
pub struct HostCapabilities {
    pub os: String,
    pub linux_ebpf: bool,
    pub windows_wfp: bool,
    pub macos_nefilter: bool,
    pub mcp_stdio: bool,
    pub mcp_http: bool,
}

impl HostCapabilities {
    pub fn degraded_reasons(&self) -> Vec<String> {
        let mut reasons = Vec::new();
        if self.os == "windows" && !self.windows_wfp {
            reasons.push("windows-wfp driver not found or not active".to_string());
        }
        if self.os == "linux" && !self.linux_ebpf {
            reasons.push("linux-ebpf requires BTF which is missing".to_string());
        }
        if self.os == "macos" && !self.macos_nefilter {
            reasons.push("macos-nefilter extension not installed".to_string());
        }
        reasons
    }
}

pub fn detect_host() -> HostCapabilities {
    let os = std::env::consts::OS;
    HostCapabilities {
        os: os.to_string(),
        linux_ebpf: cfg!(target_os = "linux") && probe_ebpf_btf(),
        windows_wfp: cfg!(target_os = "windows") && probe_wfp_active(),
        macos_nefilter: cfg!(target_os = "macos") && probe_nefilter(),
        mcp_stdio: true,
        mcp_http: true,
    }
}

fn probe_ebpf_btf() -> bool {
    std::path::Path::new("/sys/kernel/btf/vmlinux").exists()
}

fn probe_wfp_active() -> bool {
    #[cfg(target_os = "windows")]
    {
        let service_ready = ["PollekWfp", "pollek-wfp", "pollek_dek_wfp"]
            .iter()
            .any(|service| {
                std::process::Command::new("sc")
                    .args(["query", service])
                    .output()
                    .map(|output| {
                        output.status.success()
                            && String::from_utf8_lossy(&output.stdout).contains("RUNNING")
                    })
                    .unwrap_or(false)
            });
        let driver_file_ready =
            std::path::Path::new(r"C:\Windows\System32\drivers\pollek_wfp.sys").exists();
        service_ready || driver_file_ready
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

fn probe_nefilter() -> bool {
    #[cfg(target_os = "macos")]
    {
        [
            "/Library/SystemExtensions/ai.pollek.dek.networkextension.systemextension",
            "/Applications/Pollek.app/Contents/Library/SystemExtensions/ai.pollek.dek.networkextension.systemextension",
        ]
        .iter()
        .any(|path| std::path::Path::new(path).exists())
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

#[derive(Serialize, Deserialize)]
pub enum PlanMode {
    Enforcing,
    Observing,
    Mixed,
}

#[derive(Serialize)]
pub struct EnforcementPlan {
    pub intent: String,
    pub mode: PlanMode,
    pub chosen_layer: String,
    pub auto_selected: bool,
    pub friendly_th: String,
    pub fallbacks_applied: Vec<String>,
    pub user_action_th: Option<String>,
}

pub fn auto_plan(intent: &str, host: &HostCapabilities) -> EnforcementPlan {
    let (layer, mode, friendly, action) = match intent {
        "limit_network" => {
            if host.linux_ebpf {
                (
                    "linux-ebpf",
                    PlanMode::Enforcing,
                    "🛡️ บล็อกการต่อออกได้จริงด้วย eBPF",
                    None,
                )
            } else if host.macos_nefilter {
                (
                    "macos-nefilter",
                    PlanMode::Enforcing,
                    "🛡️ บล็อกได้จริงผ่าน System Extension",
                    None,
                )
            } else if host.windows_wfp {
                (
                    "windows-wfp",
                    PlanMode::Observing,
                    "👁️ ตรวจจับการต่อออกได้ แต่ยังไม่บล็อกจริงบนเครื่องนี้",
                    Some("ติดตั้ง WFP callout (Administrator) เพื่อบล็อกจริง"),
                )
            } else {
                (
                    "mcp-http",
                    PlanMode::Observing,
                    "👁️ สังเกตการณ์ผ่าน MCP",
                    Some("เปิด eBPF/NEFilter เพื่อบังคับใช้ระดับเครือข่าย"),
                )
            }
        }
        _ => {
            if host.mcp_stdio || host.mcp_http {
                (
                    "mcp-stdio",
                    PlanMode::Enforcing,
                    "🛡️ บังคับใช้ที่ชั้น MCP ได้จริง (allow/deny/redact)",
                    None,
                )
            } else {
                (
                    "none",
                    PlanMode::Observing,
                    "👁️ ยังไม่มีจุดบังคับใช้ — สังเกตการณ์ไว้ก่อน",
                    Some("ตั้งค่า agent ให้วิ่งผ่าน Pollek MCP wrapper"),
                )
            }
        }
    };
    EnforcementPlan {
        intent: intent.into(),
        mode,
        chosen_layer: layer.into(),
        auto_selected: true,
        friendly_th: friendly.into(),
        fallbacks_applied: host.degraded_reasons(),
        user_action_th: action.map(|s| s.to_string()),
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/enforcement/auto-plan", post(post_auto_plan))
        .route("/v1/enforcement/capabilities", get(get_host_capabilities))
}

#[derive(Serialize)]
pub struct HostCapabilitiesResponse {
    pub os: String,
    pub linux_ebpf: bool,
    pub windows_wfp: bool,
    pub macos_nefilter: bool,
    pub mcp_stdio: bool,
    pub mcp_http: bool,
    pub capabilities: Vec<serde_json::Value>,
}

async fn get_host_capabilities() -> ApiResult<Json<HostCapabilitiesResponse>> {
    let host = detect_host();

    let caps = vec![
        serde_json::json!({
            "pep_type": "linux_ebpf",
            "status": if host.os == "linux" && host.linux_ebpf { "available" } else { "not_installed" },
            "mode": if host.linux_ebpf { "enforce" } else { "observe_only" },
            "maturity": "enforce_beta",
            "reason": if host.os != "linux" { "not running on linux" } else if !host.linux_ebpf { "eBPF requires BTF" } else { "" }
        }),
        serde_json::json!({
            "pep_type": "windows_wfp",
            "status": if host.os == "windows" && host.windows_wfp { "available" } else if host.os == "windows" { "not_active" } else { "not_available" },
            "mode": if host.windows_wfp { "enforce" } else { "observe_only" },
            "maturity": "driver_probe_beta",
            "reason": if host.os != "windows" { "not running on windows" } else if !host.windows_wfp { "WFP driver not active" } else { "" }
        }),
        serde_json::json!({
            "pep_type": "macos_nefilter",
            "status": if host.os == "macos" && host.macos_nefilter { "available" } else if host.os == "macos" { "not_active" } else { "not_available" },
            "mode": if host.macos_nefilter { "enforce" } else { "observe_only" },
            "maturity": "system_extension_probe_beta",
            "reason": if host.os != "macos" { "not running on macOS" } else if !host.macos_nefilter { "NEFilter extension missing" } else { "" }
        }),
        serde_json::json!({
            "pep_type": "http_gateway",
            "status": "available",
            "mode": "enforce",
            "maturity": "production"
        }),
        serde_json::json!({
            "pep_type": "mcp_proxy",
            "status": "available",
            "mode": "enforce",
            "maturity": "production"
        }),
        serde_json::json!({
            "pep_type": "stdio_wrapper",
            "status": "available",
            "mode": "enforce",
            "maturity": "production"
        }),
    ];

    Ok(Json(HostCapabilitiesResponse {
        os: host.os,
        linux_ebpf: host.linux_ebpf,
        windows_wfp: host.windows_wfp,
        macos_nefilter: host.macos_nefilter,
        mcp_stdio: host.mcp_stdio,
        mcp_http: host.mcp_http,
        capabilities: caps,
    }))
}

#[derive(Deserialize)]
pub struct AutoPlanRequest {
    pub intent: String,
}

async fn post_auto_plan(Json(req): Json<AutoPlanRequest>) -> ApiResult<Json<EnforcementPlan>> {
    let host = detect_host();
    let plan = auto_plan(&req.intent, &host);
    Ok(Json(plan))
}
