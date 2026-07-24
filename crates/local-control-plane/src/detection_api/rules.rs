//! Detection rule pack: verified rule loading, coverage, and event evaluation.
use axum::{
    extract::{Path, State},
    Json,
};
use chrono::Utc;
use dek_detection::{
    build_coverage, evaluate, verify_and_load_pack, Detection, ObservedEvent, PackManifest,
    RuleSpec,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path as FsPath, PathBuf};

use super::sensors::{observe_sensors, ObserveSensor};
use crate::{error::ApiResult, state::AppState};

#[derive(Debug, Serialize)]
pub(super) struct DetectionRuleSummary {
    id: String,
    name: String,
    severity: String,
    confidence: String,
    maturity: String,
    detect_type: String,
    default_response: String,
    enforce_if_capable: Option<String>,
    observe_only_fallback: bool,
    user_message: String,
    maps: Value,
    setup_requirements: Vec<String>,
    can_stop_next_time: bool,
    privacy_note: String,
}

#[derive(Debug, Serialize)]
pub(super) struct DetectionCoverageResponse {
    schema_version: &'static str,
    tenant_id: String,
    generated_at: String,
    pack_id: String,
    pack_version: String,
    manifest_integrity: &'static str,
    rule_count: usize,
    coverage: dek_detection::Coverage,
    rules: Vec<DetectionRuleSummary>,
    sensors: Vec<ObserveSensor>,
    research_basis: Vec<ResearchBasis>,
    privacy_guards: Vec<String>,
    limitations: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct ResearchBasis {
    framework: &'static str,
    source: &'static str,
    implementation_use: &'static str,
}

#[derive(Debug, Deserialize)]
pub(super) struct EvaluateRequest {
    events: Vec<ObservedEvent>,
}

#[derive(Debug, Serialize)]
pub(super) struct DetectionHit {
    rule: DetectionRuleSummary,
    matched_event_ids: Vec<String>,
    agent_id: String,
    session_id: String,
}

#[derive(Debug, Serialize)]
pub(super) struct EvaluateResponse {
    schema_version: &'static str,
    tenant_id: String,
    evaluated_events: usize,
    fired: Vec<DetectionHit>,
}

pub(super) async fn get_coverage(
    State(state): State<AppState>,
    Path(tenant): Path<String>,
) -> ApiResult<Json<DetectionCoverageResponse>> {
    let rules = verified_rules()?;
    let coverage = build_coverage(&rules);
    let manifest = load_manifest()?;
    let sensors = observe_sensors(&state, &tenant).await;

    Ok(Json(DetectionCoverageResponse {
        schema_version: "pollek.detection.coverage.v1",
        tenant_id: tenant,
        generated_at: Utc::now().to_rfc3339(),
        pack_id: manifest.pack_id,
        pack_version: manifest.version,
        manifest_integrity: "verified",
        rule_count: rules.len(),
        coverage,
        rules: rules.iter().map(rule_summary).collect(),
        sensors,
        research_basis: research_basis(),
        privacy_guards: privacy_guards(),
        limitations: detection_limitations(),
    }))
}

pub(super) async fn list_rules(Path(_tenant): Path<String>) -> ApiResult<Json<Value>> {
    let rules = verified_rules()?;
    Ok(Json(json!({
        "schema_version": "pollek.detection.rules.v1",
        "pack": load_manifest()?,
        "items": rules.iter().map(rule_summary).collect::<Vec<_>>()
    })))
}

pub(super) async fn evaluate_events(
    Path(tenant): Path<String>,
    Json(req): Json<EvaluateRequest>,
) -> ApiResult<Json<EvaluateResponse>> {
    let rules = verified_rules()?;
    let mut groups: BTreeMap<(String, String), Vec<ObservedEvent>> = BTreeMap::new();
    let evaluated_events = req.events.len();

    for event in req.events {
        groups
            .entry((event.agent_id.clone(), event.session_id.clone()))
            .or_default()
            .push(event);
    }

    let mut fired = Vec::new();
    for ((agent_id, session_id), mut events) in groups {
        events.sort_by_key(|event| event.ts_ms);
        for rule in &rules {
            if let Some(hit) = evaluate(rule, &events) {
                fired.push(hit_response(rule, hit, &agent_id, &session_id));
            }
        }
    }

    Ok(Json(EvaluateResponse {
        schema_version: "pollek.detection.evaluate.v1",
        tenant_id: tenant,
        evaluated_events,
        fired,
    }))
}

fn hit_response(
    rule: &RuleSpec,
    detection: Detection,
    agent_id: &str,
    session_id: &str,
) -> DetectionHit {
    DetectionHit {
        rule: rule_summary(rule),
        matched_event_ids: detection.matched_event_ids,
        agent_id: agent_id.to_string(),
        session_id: session_id.to_string(),
    }
}

fn verified_rules() -> ApiResult<Vec<RuleSpec>> {
    let pack_dir = detection_pack_dir();
    verify_and_load_pack(pack_dir, |_manifest, _dir| Ok(()))
        .map_err(|err| crate::error::ApiError::Internal(anyhow::anyhow!(err)))
}

fn load_manifest() -> ApiResult<PackManifest> {
    let path = detection_pack_dir().join("manifest.json");
    let text = std::fs::read_to_string(&path).map_err(|err| {
        crate::error::ApiError::Internal(anyhow::anyhow!(
            "failed to read detection manifest {}: {err}",
            path.display()
        ))
    })?;
    serde_json::from_str(&text).map_err(|err| {
        crate::error::ApiError::Internal(anyhow::anyhow!(
            "failed to parse detection manifest {}: {err}",
            path.display()
        ))
    })
}

fn detection_pack_dir() -> PathBuf {
    if let Ok(path) = std::env::var("POLLEK_DETECTION_PACK_DIR") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    let mut candidates = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("contracts/detections/packs/core-v1"));
        candidates.push(cwd.join("../contracts/detections/packs/core-v1"));
    }
    candidates.push(
        FsPath::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts/detections/packs/core-v1"),
    );

    candidates
        .iter()
        .find(|path| path.join("manifest.json").exists())
        .cloned()
        .unwrap_or_else(|| {
            FsPath::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts/detections/packs/core-v1")
        })
}

fn rule_summary(rule: &RuleSpec) -> DetectionRuleSummary {
    let enforce = rule.response.enforce_if_capable;
    DetectionRuleSummary {
        id: rule.id.clone(),
        name: rule.name.clone(),
        severity: enum_name(&rule.severity),
        confidence: enum_name(&rule.confidence),
        maturity: enum_name(&rule.maturity),
        detect_type: enum_name(&rule.detect.detect_type),
        default_response: enum_name(&rule.response.default),
        enforce_if_capable: enforce.map(|action| enum_name(&action)),
        observe_only_fallback: rule.response.observe_only_fallback,
        user_message: rule.response.user_message.clone(),
        maps: serde_json::to_value(&rule.maps).unwrap_or_else(|_| json!({})),
        setup_requirements: setup_requirements_for_rule(rule),
        can_stop_next_time: enforce.is_some(),
        privacy_note: "Detection uses redacted metadata and rule IDs. It does not store raw prompt, response, email body, or file content.".into(),
    }
}

fn enum_name<T: std::fmt::Debug>(value: &T) -> String {
    format!("{value:?}").to_ascii_lowercase()
}

fn setup_requirements_for_rule(rule: &RuleSpec) -> Vec<String> {
    let mut out = Vec::new();
    for step in &rule.detect.steps {
        if let Some(activity) = &step.activity {
            match activity.as_str() {
                "FileRead" | "PackageInstall" => out.push(
                    "File/process visibility needs local OS metadata, MCP wrapper, SDK wrapper, or structured agent logs.".into(),
                ),
                "WebUpload" | "WebVisit" => out.push(
                    "Browser or network visibility needs browser connector, HTTP/MCP proxy, WFP, Network Extension, or eBPF.".into(),
                ),
                "ShellCommand" => out.push(
                    "Command execution visibility needs terminal wrapper, MCP tool proxy, process audit, or agent SDK hook.".into(),
                ),
                "LlmApiCall" => out.push(
                    "LLM usage visibility needs provider usage object, SDK wrapper, browser connector, MCP/HTTP proxy, or local log source.".into(),
                ),
                _ => {}
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn research_basis() -> Vec<ResearchBasis> {
    vec![
        ResearchBasis {
            framework: "OWASP Top 10 for LLM Applications",
            source: "https://genai.owasp.org/llm-top-10/",
            implementation_use: "Rule mappings for prompt injection, sensitive disclosure, supply chain, excessive agency, and unbounded consumption.",
        },
        ResearchBasis {
            framework: "NIST AI RMF / Generative AI Profile",
            source: "https://doi.org/10.6028/NIST.AI.600-1",
            implementation_use: "Risk mapping, measurement, governance traceability, and user disclosure for AI activity monitoring.",
        },
        ResearchBasis {
            framework: "NIST SSDF",
            source: "https://csrc.nist.gov/Projects/ssdf",
            implementation_use: "Signed content, manifest integrity, and repeatable CI checks for detection rule packs.",
        },
        ResearchBasis {
            framework: "EDR-style local sensors",
            source: "OS vendor APIs: WFP, Endpoint Security, Network Extension, eBPF, fanotify, and browser extensions.",
            implementation_use: "Capability probes and setup gates keep OS-level enforcement honest and observable-first.",
        },
    ]
}

fn privacy_guards() -> Vec<String> {
    vec![
        "No raw prompt, response, email body, or file content is stored by detection rules.".into(),
        "Rules operate on redacted metadata, classifications, hashes, timestamps, and provenance tags.".into(),
        "Browser prompt checking requires explicit extension approval; exact body capture is not enabled silently.".into(),
        "Enterprise third-party NER remains an explicit future routing point with provider consent and audit metadata.".into(),
    ]
}

fn detection_limitations() -> Vec<String> {
    vec![
        "Kernel-level enforcement depends on OS support, signed components, user or admin approval, and warm checks.".into(),
        "Encrypted HTTPS metadata alone cannot reveal prompt or response bodies; use browser extension, SDK wrapper, MCP proxy, or gateway for plaintext guard paths.".into(),
        "Observe-only fallback stays available when a native driver, extension, entitlement, or privilege is missing.".into(),
        "A local dashboard can guide installation and record consent, but browsers and operating systems intentionally require their own approval prompts for privileged components.".into(),
    ]
}
