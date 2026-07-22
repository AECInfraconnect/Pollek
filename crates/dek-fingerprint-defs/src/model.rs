use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedDefinition {
    pub payload: FingerprintDefinition,
    pub signature: String, // base64 encoded ed25519 signature
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FingerprintDefinition {
    pub schema_version: String,
    pub definition_version: u64,
    pub released_at: String,
    pub min_engine_version: String,
    pub kind: DefinitionKind,
    pub base_version: Option<u64>,
    pub signatures: Vec<AgentSignatureV2>,
    pub removed_ids: Vec<String>,
    pub catalog_hash: String,
    #[serde(default)]
    pub model_classifier: Option<ModelClassifierDef>,
    #[serde(default)]
    pub web_ai_signatures: Vec<WebAiSignatureDef>,
    #[serde(default)]
    pub installed_app_signatures: Vec<InstalledAppSignatureDef>,
    #[serde(default)]
    pub browser_processes: Vec<BrowserProcessDef>,
    #[serde(default)]
    pub ai_process_hints: AiProcessHints,
    #[serde(default)]
    pub cloud_resource_signatures: Vec<CloudResourceSignatureDef>,
    pub collapse_rules: Vec<CollapseRuleDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CloudResourceSignatureDef {
    pub host_pattern: String,
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub classification: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowserProcessDef {
    #[serde(default)]
    pub process_names: Vec<String>,
    pub engine: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct AiProcessHints {
    #[serde(default)]
    pub require_match: bool,
    #[serde(default)]
    pub name_tokens: Vec<String>,
    #[serde(default)]
    pub cmd_tokens: Vec<String>,
    #[serde(default)]
    pub deny_tokens: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledAppSignatureDef {
    #[serde(default)]
    pub id: String,
    #[serde(alias = "display_name")]
    pub name: String,
    pub vendor: String,
    #[serde(default)]
    pub product: String,
    #[serde(default)]
    pub agent_type: String,
    #[serde(default)]
    pub capability_tags: Vec<String>,
    #[serde(default)]
    pub process_names: Vec<String>,
    #[serde(default)]
    pub markers: Vec<InstalledAppMarker>,
}

impl InstalledAppSignatureDef {
    /// All process names this app can surface under: the top-level list plus any
    /// declared per-marker (per-OS) process names. Extension/plugin agents keep
    /// their names on the OS marker, so the matcher must see both.
    pub fn process_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.process_names.clone();
        for m in &self.markers {
            for n in &m.process_names {
                if !names
                    .iter()
                    .any(|existing| existing.eq_ignore_ascii_case(n))
                {
                    names.push(n.clone());
                }
            }
        }
        names
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledAppMarker {
    pub os: Option<String>,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub process_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebAiSignatureDef {
    pub id: String,
    pub canonical_service_id: String,
    pub surface_group_id: String,
    pub entity_role: String,
    pub authority_boundary: String,
    pub domain: String,
    pub alias_domains: Vec<String>,
    pub related_domains: Vec<String>,
    pub not_alias_domains: Vec<String>,
    pub exclusive_match: bool,
    pub parent_precedence: Vec<String>,
    pub observe_scope: String,
    pub enforce_scope: String,
    pub ui_class: String,
    pub name: String,
    pub vendor: String,
    #[serde(default)]
    pub title_patterns: Vec<String>,
    #[serde(default)]
    pub app_cmdline_patterns: Vec<String>,
    #[serde(default)]
    pub capability_tags: Vec<String>,
    #[serde(default = "default_web_risk")]
    pub risk_weight: f64,
}

impl WebAiSignatureDef {
    pub fn stable_id(&self) -> &str {
        if self.id.is_empty() {
            &self.domain
        } else {
            &self.id
        }
    }

    pub fn domains(&self) -> Vec<&str> {
        let mut domains = vec![self.domain.as_str()];
        domains.extend(self.alias_domains.iter().map(String::as_str));
        domains.sort_unstable();
        domains.dedup();
        domains
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CollapseRuleDef {
    pub id: String,
    pub when_parent_signature_id: Option<String>,
    pub when_endpoint: Option<String>,
    pub child_service_ids: Vec<String>,
    pub parent_client_candidates: Vec<String>,
    pub same_window_seconds: Option<u64>,
    pub same_user_or_profile_required: bool,
    pub collapse_as: String,
    pub control_parent_only: bool,
    pub keep_child_visible: bool,
}

fn default_web_risk() -> f64 {
    0.4
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelClassifierDef {
    #[serde(default)]
    pub vendors: Vec<VendorDef>,
    #[serde(default)]
    pub family_rules: Vec<FamilyRuleDef>,
    #[serde(default)]
    pub attribute_parsers: BTreeMap<String, AttributeParserDef>,
    #[serde(default)]
    pub risk_flags: Vec<RiskFlagDef>,
    #[serde(default)]
    pub popular_models: Vec<PopularModelDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VendorDef {
    pub ns: Vec<String>,
    pub vendor: String,
    pub license_class: Option<String>,
    #[serde(default)]
    pub flags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FamilyRuleDef {
    pub id: String,
    pub pattern: String,
    pub family: String,
    pub vendor: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub risk_base: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AttributeParserDef {
    String(String),
    Map(BTreeMap<String, String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskFlagDef {
    pub pattern: String,
    pub flag: String,
    pub risk_add: f64,
    #[serde(default)]
    pub tags: Vec<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PopularModelDef {
    #[serde(default)]
    pub match_pattern: String,
    #[serde(alias = "match")]
    pub match_: Option<String>, // the JSON has `match` instead of `match_pattern`, so we use alias and fallback
    pub display: String,
    pub vendor: Option<String>,
    pub family: String,
    pub license: Option<String>,
    pub arch: Option<String>,
    pub params_total_b: Option<f64>,
    pub params_active_b: Option<f64>,
    pub context: Option<u64>,
    #[serde(default)]
    pub modality: Vec<String>,
    pub popularity: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub risk_base: f64,
    #[serde(default)]
    pub flags: Vec<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelClass {
    pub raw_id: String,
    pub display: String,
    pub vendor: Option<String>,
    pub family: String,
    pub license: Option<String>,
    pub arch: Option<String>,
    pub params_total_b: Option<f64>,
    pub params_active_b: Option<f64>,
    pub context: Option<u64>,
    pub modality: Vec<String>,
    pub quant: Option<String>,
    pub variant: Vec<String>,
    pub capability_tags: Vec<String>,
    pub risk_score: f64,
    pub flags: Vec<String>,
    pub matched_tier: &'static str,
    pub needs_human: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassBase {
    pub display: String,
    pub family: String,
    pub vendor: Option<String>,
    pub license: Option<String>,
    pub arch: Option<String>,
    pub params_total_b: Option<f64>,
    pub params_active_b: Option<f64>,
    pub context: Option<u64>,
    pub modality: Vec<String>,
    pub quant: Option<String>,
    pub variant: Vec<String>,
    pub capability_tags: Vec<String>,
    pub risk_base: f64,
    pub flags: Vec<String>,
    pub matched_tier: &'static str,
}

impl ClassBase {
    pub fn unknown(vendor: Option<String>) -> Self {
        Self {
            display: "Unknown Model".into(),
            family: "unknown".into(),
            vendor,
            license: None,
            arch: None,
            params_total_b: None,
            params_active_b: None,
            context: None,
            modality: vec!["text".into()],
            quant: None,
            variant: vec![],
            capability_tags: vec![],
            risk_base: 0.4,
            flags: vec![],
            matched_tier: "unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DefinitionKind {
    Full,
    Delta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSignatureV2 {
    pub id: String,
    pub display_name: String,
    pub agent_type: String,
    #[serde(default = "default_revision")]
    pub revision: u32,
    #[serde(default)]
    pub meta: SignatureMeta,

    #[serde(default)]
    pub process_names: Vec<String>,
    #[serde(default)]
    pub binary_hashes: Vec<String>,
    #[serde(default)]
    pub config_paths: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub config_parsers: Vec<String>,
    #[serde(default)]
    pub ports: Vec<u16>,
    #[serde(default)]
    pub port_probe: Option<PortProbeSpec>,
    #[serde(default = "default_detection_logic")]
    pub detection_logic: DetectionLogic,

    #[serde(default)]
    pub control_strategies: Vec<String>,
    #[serde(default = "default_agent_risk")]
    pub risk_weight: f64,

    // Extra signals reduce ambiguity for wrapper processes such as node.exe.
    #[serde(default)]
    pub cmd_patterns: Vec<String>,
    #[serde(default)]
    pub exe_path_patterns: Vec<String>,
    #[serde(default)]
    pub install_markers: Vec<InstallMarker>,
    #[serde(default)]
    pub cli_binaries: Vec<String>,
    #[serde(default)]
    pub package_markers: Vec<PackageMarker>,
    #[serde(default)]
    pub env_markers: Vec<String>,
    #[serde(default)]
    pub egress_hosts: Vec<String>,
    #[serde(default)]
    pub vendor: Option<String>,
    #[serde(default)]
    pub product: Option<String>,
    #[serde(default)]
    pub capability_tags: Vec<String>,
    #[serde(default)]
    pub signal_weights: Option<SignalWeights>,
}

fn default_revision() -> u32 {
    1
}

fn default_agent_risk() -> f64 {
    0.5
}

fn default_detection_logic() -> DetectionLogic {
    DetectionLogic::AnyOf
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallMarker {
    pub path: String,
    pub os: Option<String>,
    pub weight: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageMarker {
    pub ecosystem: String,
    pub name: String,
    pub global: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalWeights {
    pub process_name: f64,
    pub cmd_pattern: f64,
    pub exe_path: f64,
    pub install_marker: f64,
    pub cli_binary: f64,
    pub package: f64,
    pub config_path: f64,
    pub port: f64,
    pub egress: f64,
    pub binary_hash: f64,
}

impl Default for SignalWeights {
    fn default() -> Self {
        Self {
            process_name: 0.15,
            cmd_pattern: 0.45,
            exe_path: 0.40,
            install_marker: 0.55,
            cli_binary: 0.50,
            package: 0.45,
            config_path: 0.50,
            port: 0.25,
            egress: 0.30,
            binary_hash: 0.95,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SignatureMeta {
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub references: Vec<String>,
    #[serde(default)]
    pub added_in: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortProbeSpec {
    pub kind: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DetectionLogic {
    AnyOf,
    ProcessAndConfig,
    ProcessOrConfigWithPort,
    ProcessOrPort,
    HashMatch,
    NetworkEgress,
}
