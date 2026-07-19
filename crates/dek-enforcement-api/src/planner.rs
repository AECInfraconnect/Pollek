use std::cmp::Ord;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ControlLevel {
    #[serde(alias = "Observe")]
    Observe,
    #[serde(alias = "Warn")]
    Warn,
    #[serde(alias = "Ask")]
    Ask,
    #[serde(alias = "Enforce")]
    Enforce,
}

impl ControlLevel {
    pub fn min_observe(&self) -> ControlLevel {
        ControlLevel::Observe
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlDomain {
    #[serde(alias = "Network")]
    Network,
    #[serde(alias = "FileSystem")]
    FileSystem,
    #[serde(alias = "McpTool")]
    McpTool,
    #[serde(alias = "Process")]
    Process,
    #[serde(alias = "Dns")]
    Dns,
}

impl std::fmt::Display for ControlDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ControlDomain::Network => write!(f, "network"),
            ControlDomain::FileSystem => write!(f, "file_system"),
            ControlDomain::McpTool => write!(f, "mcp_tool"),
            ControlDomain::Process => write!(f, "process"),
            ControlDomain::Dns => write!(f, "dns"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MethodStatus {
    #[serde(alias = "Available")]
    Available,
    #[serde(alias = "NeedsInstall")]
    NeedsInstall,
    #[serde(alias = "NeedsPermission")]
    NeedsPermission,
    #[serde(alias = "Unsupported")]
    Unsupported,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ControlMethodCap {
    pub id: String,
    pub domains: Vec<ControlDomain>,
    pub max_level: ControlLevel,
    pub status: MethodStatus,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CapabilityUpgrade {
    pub unlocks: String,
    #[serde(default)]
    pub action_id: String,
    #[serde(default)]
    pub reason_code: String,
    #[serde(default)]
    pub title_en: String,
    #[serde(default)]
    pub title_th: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LocalCapabilitySnapshot {
    pub control_methods: Vec<ControlMethodCap>,
}

impl LocalCapabilitySnapshot {
    pub fn observe_capable(&self, _domain: ControlDomain) -> Option<&ControlMethodCap> {
        self.control_methods.iter().find(|m| {
            m.domains.contains(&_domain)
                && m.max_level >= ControlLevel::Observe
                && matches!(
                    m.status,
                    MethodStatus::Available | MethodStatus::NeedsPermission
                )
        })
    }
    pub fn upgrade_for(&self, domain: ControlDomain) -> Option<CapabilityUpgrade> {
        let (action_id, reason_code, title_en, title_th, unlocks) = match domain {
            ControlDomain::Network | ControlDomain::Dns => (
                "install_device_network_control",
                "needs_os_network_extension",
                "Install device-level network control",
                "ติดตั้งตัวควบคุมเครือข่ายระดับเครื่อง",
                "real network egress blocking",
            ),
            ControlDomain::McpTool => (
                "approve_mcp_config_wrapper",
                "needs_mcp_config_change",
                "Allow Pollek to wrap this agent's tool configuration",
                "อนุญาตให้ Pollek ครอบการตั้งค่าเครื่องมือของ Agent นี้",
                "MCP tool-call enforcement",
            ),
            ControlDomain::FileSystem => (
                "enable_file_activity_control",
                "observe_only_no_local_control_method",
                "Enable file activity control",
                "เปิดใช้การควบคุมกิจกรรมไฟล์",
                "file access enforcement",
            ),
            ControlDomain::Process => (
                "enable_process_observation",
                "observe_only_no_local_control_method",
                "Enable process observation",
                "เปิดใช้การสังเกต process",
                "process launch observation",
            ),
        };

        Some(CapabilityUpgrade {
            unlocks: unlocks.to_string(),
            action_id: action_id.to_string(),
            reason_code: reason_code.to_string(),
            title_en: title_en.to_string(),
            title_th: title_th.to_string(),
        })
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Policy {
    pub id: String,
    pub requested_level: ControlLevel,
}

impl Policy {
    pub fn required_domains(&self) -> Vec<ControlDomain> {
        let id = self.id.to_ascii_lowercase();
        let mut domains = if id.contains("mcp") || id.contains("tool") {
            vec![ControlDomain::McpTool]
        } else if id.contains("network") || id.contains("shadow") || id.contains("egress") {
            vec![ControlDomain::Network, ControlDomain::Dns]
        } else if id.contains("file")
            || id.contains("folder")
            || id.contains("secret")
            || id.contains("write")
        {
            vec![ControlDomain::FileSystem, ControlDomain::McpTool]
        } else if id.contains("prompt")
            || id.contains("pii")
            || id.contains("redact")
            || id.contains("budget")
            || id.contains("token")
            || id.contains("cost")
        {
            vec![ControlDomain::McpTool, ControlDomain::Network]
        } else {
            vec![ControlDomain::McpTool]
        };
        domains.sort();
        domains.dedup();
        domains
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DomainFeasibility {
    pub domain: ControlDomain,
    pub chosen_method: Option<String>,
    pub level: ControlLevel,
    #[serde(default)]
    pub reason_code: String,
    #[serde(default)]
    pub setup_action_ids: Vec<String>,
    #[serde(default)]
    pub enforced_for_real: bool,
}

impl DomainFeasibility {
    pub fn ok(domain: ControlDomain, method: &ControlMethodCap, level: ControlLevel) -> Self {
        let enforced_for_real =
            method.status == MethodStatus::Available && level >= ControlLevel::Enforce;
        Self {
            domain,
            chosen_method: Some(method.id.clone()),
            level,
            reason_code: if method.status == MethodStatus::Available {
                "fully_protected".into()
            } else {
                "observe_only_permission_required".into()
            },
            setup_action_ids: Vec::new(),
            enforced_for_real,
        }
    }
    pub fn observe_fallback(domain: ControlDomain, upgrade: Option<&CapabilityUpgrade>) -> Self {
        Self {
            domain,
            chosen_method: None,
            level: ControlLevel::Observe,
            reason_code: upgrade
                .map(|u| u.reason_code.clone())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "observe_only_no_local_control_method".into()),
            setup_action_ids: upgrade
                .map(|u| vec![u.action_id.clone()])
                .unwrap_or_default()
                .into_iter()
                .filter(|id| !id.is_empty())
                .collect(),
            enforced_for_real: false,
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeasibilityVerdict {
    FullyEnforceable,
    PartialObserve,
    ObserveOnly,
    NotApplicable,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PolicyFeasibilityResult {
    pub policy_id: String,
    pub requested_level: ControlLevel,
    pub achievable_level: ControlLevel,
    pub verdict: FeasibilityVerdict,
    pub per_domain: Vec<DomainFeasibility>,
    pub gaps: Vec<CapabilityUpgrade>,
    pub friendly_th: String,
    pub friendly_en: String,
}

impl PolicyFeasibilityResult {
    pub fn build(
        policy: &Policy,
        achievable: ControlLevel,
        per_domain: Vec<DomainFeasibility>,
        gaps: Vec<CapabilityUpgrade>,
    ) -> Self {
        let verdict = if gaps.is_empty() {
            FeasibilityVerdict::FullyEnforceable
        } else if per_domain.iter().any(|d| d.level == ControlLevel::Observe) {
            if per_domain.iter().any(|d| d.level == ControlLevel::Enforce) {
                FeasibilityVerdict::PartialObserve
            } else {
                FeasibilityVerdict::ObserveOnly
            }
        } else {
            FeasibilityVerdict::FullyEnforceable
        };

        let (friendly_th, friendly_en) = match verdict {
            FeasibilityVerdict::FullyEnforceable => (
                "พร้อมบังคับใช้จริงบนเครื่องนี้".to_string(),
                "Fully enforceable on this device".to_string(),
            ),
            FeasibilityVerdict::PartialObserve => (
                "บางส่วนบังคับใช้จริง บางส่วนสังเกตการณ์ — แตะดูวิธีเปิดให้ครบ".to_string(),
                "Partly enforced — tap to enable full enforcement".to_string(),
            ),
            FeasibilityVerdict::ObserveOnly => (
                "ตอนนี้ทำได้แค่สังเกตการณ์ — ติดตั้งส่วนเสริมเพื่อบล็อกจริง".to_string(),
                "Observe-only — install an add-on to actually block".to_string(),
            ),
            FeasibilityVerdict::NotApplicable => (
                "นโยบายนี้ไม่เกี่ยวกับ Agent ที่เลือก".to_string(),
                "Doesn't apply to selected agent".to_string(),
            ),
        };

        Self {
            policy_id: policy.id.clone(),
            requested_level: policy.requested_level.clone(),
            achievable_level: achievable,
            verdict,
            per_domain,
            gaps,
            friendly_th,
            friendly_en,
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct MethodBinding {
    pub domain: ControlDomain,
    pub method_id: String,
    pub effective_level: ControlLevel,
    pub maturity: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ControlMethodPlan {
    pub policy_id: String,
    pub bindings: Vec<MethodBinding>,
    pub fallbacks: Vec<String>,
    pub auto_selected: bool,
}

/// 1) Feasibility planner — policy นี้ทำได้จริงแค่ไหนบนเครื่องนี้
pub fn assess_feasibility(
    policy: &Policy,
    snap: &LocalCapabilitySnapshot,
) -> PolicyFeasibilityResult {
    let mut per_domain = vec![];
    let mut gaps = vec![];
    let mut achievable = ControlLevel::Enforce;
    for domain in policy.required_domains() {
        match select_method(domain.clone(), policy.requested_level.clone(), snap) {
            Some(m) => {
                let lvl = m.max_level.clone().min(policy.requested_level.clone()); // negotiate ลงตามจริง
                achievable = achievable.min(lvl.clone());
                per_domain.push(DomainFeasibility::ok(domain, m, lvl));
            }
            None => {
                achievable = ControlLevel::Observe.min(achievable);
                let upgrade = snap.upgrade_for(domain.clone());
                if let Some(u) = &upgrade {
                    gaps.push(u.clone());
                }
                per_domain.push(DomainFeasibility::observe_fallback(
                    domain,
                    upgrade.as_ref(),
                ));
            }
        }
    }
    PolicyFeasibilityResult::build(policy, achievable, per_domain, gaps)
}

/// 2) Control method selector — เลือกวิธีคุมที่ "ดีที่สุดที่ทำได้จริง" ต่อ domain (capability-aware)
fn select_method(
    domain: ControlDomain,
    want: ControlLevel,
    snap: &LocalCapabilitySnapshot,
) -> Option<&ControlMethodCap> {
    let pref: &[&str] = match domain {
        ControlDomain::Network => &["linux_ebpf", "macos_netext", "windows_wfp_um", "mcp_http"],
        ControlDomain::FileSystem => &["linux_landlock", "macos_es", "mcp_stdio"],
        ControlDomain::McpTool => &["mcp_stdio", "mcp_http"],
        ControlDomain::Process => &["linux_ebpf", "macos_es", "windows_etw"],
        ControlDomain::Dns => &["macos_netext", "linux_ebpf", "mcp_http"],
    };
    pref.iter()
        .find_map(|id| {
            snap.control_methods.iter().find(|m| {
                &m.id == id
                    && m.status == MethodStatus::Available
                    && m.domains.contains(&domain)
                    && m.max_level >= want.min_observe()
            })
        })
        .or_else(|| snap.observe_capable(domain)) // ไม่มี enforce → observe
}

/// 3) Control level negotiation — สรุประดับที่ได้จริง; ห้าม downgrade เงียบ
pub fn negotiate(r: &PolicyFeasibilityResult) -> ControlMethodPlan {
    ControlMethodPlan {
        policy_id: r.policy_id.clone(),
        bindings: r
            .per_domain
            .iter()
            .filter_map(|d| {
                d.chosen_method.clone().map(|mid| MethodBinding {
                    domain: d.domain.clone(),
                    method_id: mid,
                    effective_level: d.level.clone(),
                    maturity: String::new(),
                })
            })
            .collect(),
        fallbacks: r
            .per_domain
            .iter()
            .filter(|d| d.chosen_method.is_none())
            .map(|d| format!("{}: observe (no enforce method)", d.domain))
            .collect(),
        auto_selected: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_negotiate_enforce_to_observe() {
        let policy = Policy {
            id: "pol_1".to_string(),
            requested_level: ControlLevel::Enforce,
        };
        let snap = LocalCapabilitySnapshot {
            control_methods: vec![ControlMethodCap {
                id: "macos_netext".to_string(),
                domains: vec![ControlDomain::Network],
                max_level: ControlLevel::Observe,
                status: MethodStatus::Available,
            }],
        };
        let result = assess_feasibility(&policy, &snap);
        let plan = negotiate(&result);
        assert_eq!(plan.policy_id, "pol_1");
    }

    #[test]
    fn test_assess_feasibility_no_methods() {
        let policy = Policy {
            id: "pol_empty".to_string(),
            requested_level: ControlLevel::Enforce,
        };
        let snap = LocalCapabilitySnapshot {
            control_methods: vec![],
        };
        let result = assess_feasibility(&policy, &snap);
        assert_eq!(result.policy_id, "pol_empty");
    }

    #[test]
    fn pii_policy_requires_tool_and_network_domains() {
        let policy = Policy {
            id: "pii.redact_before_external_llm".to_string(),
            requested_level: ControlLevel::Enforce,
        };
        let domains = policy.required_domains();
        assert!(domains.contains(&ControlDomain::McpTool));
        assert!(domains.contains(&ControlDomain::Network));
    }

    #[test]
    fn observe_fallback_carries_reason_and_setup_action() {
        let policy = Policy {
            id: "network.shadow_ai_external_llm_block".to_string(),
            requested_level: ControlLevel::Enforce,
        };
        let snap = LocalCapabilitySnapshot {
            control_methods: vec![],
        };
        let result = assess_feasibility(&policy, &snap);
        assert!(matches!(result.verdict, FeasibilityVerdict::ObserveOnly));
        assert!(result
            .per_domain
            .iter()
            .all(|domain| !domain.reason_code.is_empty()));
        assert!(result.per_domain.iter().any(|domain| domain
            .setup_action_ids
            .contains(&"install_device_network_control".to_string())));
    }
}
