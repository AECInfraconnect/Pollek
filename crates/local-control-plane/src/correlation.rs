//! Agent signal-correlation: the SSOT projection from discovered agents to
//! process identity, plus a live-resolve API that attributes the currently
//! running processes to known agents.
//!
//! Phase 1 of the observe/enforce roadmap. Low-level sensors (eBPF ring buffer,
//! Windows ETW, macOS EndpointSecurity) see *processes and flows*; the agent
//! identity lives in user space, discovered by [`dek_agent_discovery`]. This
//! module is the single place that projects a discovered agent's process
//! evidence into an [`AgentProcessBinding`], so both the ingest path (stamping
//! agent-less observation events) and the dashboard read the *same* mapping.
//!
//! The canonical agent id for a candidate is resolved through the existing
//! [`crate::agent_discovery_api::registered_agent_id_for_candidate`] linkage —
//! a registered agent's real `agent_id` when it exists, otherwise the stable
//! candidate key — so correlation and registration never diverge.

use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use dek_agent_discovery::model::{DiscoveredAgentCandidateV2, DiscoveryEvidenceV2, EvidenceSource};
use dek_agent_discovery::process_scan::ProcessEvidence;
use dek_agent_observer::agent_correlator::{AgentCorrelator, AgentProcessBinding};
use dek_agent_observer::model::ProcessSignal;
use serde_json::json;

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/tenants/:tenant/correlation", get(correlation_status))
}

/// Extract the process identity (pids, executable hash, process names) a
/// candidate carries from its `ProcessScan` evidence. Mirrors the extraction
/// used by the discovery aggregator (`ev.data.process` or `ev.data`) so the
/// keys hash identically to a raw runtime signal.
pub fn process_identity_from_evidence(
    evidence: &[DiscoveryEvidenceV2],
) -> (Vec<u32>, Option<String>, Vec<String>) {
    let mut pids = Vec::new();
    let mut exe_path_hash = None;
    let mut process_names = Vec::new();
    for ev in evidence {
        if ev.source != EvidenceSource::ProcessScan {
            continue;
        }
        let process_data = ev.data.get("process").unwrap_or(&ev.data);
        if let Ok(p) = serde_json::from_value::<ProcessEvidence>(process_data.clone()) {
            if !pids.contains(&p.pid) {
                pids.push(p.pid);
            }
            if exe_path_hash.is_none() {
                if let Some(h) = p.exe_path_hash.filter(|h| !h.is_empty()) {
                    exe_path_hash = Some(h);
                }
            }
            let name = p.process_name.trim().to_string();
            if !name.is_empty() && !process_names.contains(&name) {
                process_names.push(name);
            }
        }
    }
    (pids, exe_path_hash, process_names)
}

/// Project one discovered candidate into an [`AgentProcessBinding`] under the
/// given canonical `agent_id`. Returns `None` when the candidate carries no
/// usable process identity (e.g. a browser-only or remote surface).
pub fn binding_from_candidate(
    candidate: &DiscoveredAgentCandidateV2,
    agent_id: &str,
) -> Option<AgentProcessBinding> {
    let (pids, exe_path_hash, process_names) = process_identity_from_evidence(&candidate.evidence);
    if pids.is_empty() && exe_path_hash.is_none() && process_names.is_empty() {
        return None;
    }
    Some(AgentProcessBinding {
        agent_id: agent_id.to_string(),
        pids,
        exe_path_hash,
        process_names,
        cgroup_ids: Vec::new(),
    })
}

/// Load every discovery candidate for the tenant and project it into an
/// [`AgentProcessBinding`] under its canonical agent id (registered id when the
/// agent has been registered, otherwise the stable candidate key).
pub async fn build_bindings(
    st: &AppState,
    tenant: &str,
) -> anyhow::Result<Vec<AgentProcessBinding>> {
    let raw = st
        .registry_store
        .list_raw(tenant, "discovery_candidate")
        .await?;
    let mut bindings = Vec::new();
    for value in raw {
        let candidate: DiscoveredAgentCandidateV2 = match serde_json::from_value(value) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let agent_id =
            crate::agent_discovery_api::registered_agent_id_for_candidate(st, tenant, &candidate)
                .await?
                .unwrap_or_else(|| dek_agent_discovery::stable_agent_key(&candidate));
        if let Some(binding) = binding_from_candidate(&candidate, &agent_id) {
            bindings.push(binding);
        }
    }
    Ok(bindings)
}

/// Build the live correlator for the tenant from the current SSOT bindings.
pub async fn build_correlator(st: &AppState, tenant: &str) -> anyhow::Result<AgentCorrelator> {
    let bindings = build_bindings(st, tenant).await?;
    Ok(AgentCorrelator::from_bindings(&bindings))
}

/// `GET /v1/tenants/:tenant/correlation` — the live signal-correlation view:
/// the SSOT bindings, plus a real scan of the currently running processes with
/// each one attributed (or not) to a known agent. Runs unprivileged.
async fn correlation_status(
    State(state): State<AppState>,
    Path(tenant): Path<String>,
) -> impl IntoResponse {
    let bindings = match build_bindings(&state, &tenant).await {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
        }
    };
    let correlator = AgentCorrelator::from_bindings(&bindings);

    let processes = dek_agent_discovery::process_scan::scan_processes().unwrap_or_default();
    let processes_scanned = processes.len();

    let mut attributions = Vec::new();
    for p in &processes {
        let signal = ProcessSignal {
            pid: Some(p.pid),
            process_name: Some(p.process_name.clone()),
            exe_path_hash: p.exe_path_hash.clone(),
            cgroup_id: None,
            remote_addr: None,
            remote_port: None,
        };
        if let Some(resolution) = correlator.resolve(&signal) {
            attributions.push(json!({
                "pid": p.pid,
                "process_name": p.process_name,
                "exe_path_redacted": p.exe_path_redacted,
                "agent_id": resolution.agent_id,
                "basis": basis_label(resolution.basis),
                "confidence": resolution.confidence,
            }));
        }
    }

    let bindings_json: Vec<_> = bindings
        .iter()
        .map(|b| {
            json!({
                "agent_id": b.agent_id,
                "pids": b.pids,
                "exe_path_hash": b.exe_path_hash,
                "process_names": b.process_names,
                "cgroup_ids": b.cgroup_ids,
            })
        })
        .collect();

    (
        StatusCode::OK,
        Json(json!({
            "schema_version": "signal-correlation.v1",
            "tenant_id": tenant,
            "generated_at": chrono::Utc::now().to_rfc3339(),
            "agents_indexed": bindings.len(),
            "bindings": bindings_json,
            "live_scan": {
                "processes_scanned": processes_scanned,
                "attributed": attributions.len(),
                "attributions": attributions,
            },
        })),
    )
}

fn basis_label(basis: dek_agent_observer::agent_correlator::MatchBasis) -> &'static str {
    use dek_agent_observer::agent_correlator::MatchBasis::*;
    match basis {
        PidAndExe => "pid_and_exe",
        ExeHash => "exe_hash",
        Cgroup => "cgroup",
        Pid => "pid",
        ProcessNameUnique => "process_name_unique",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dek_agent_discovery::model::{EvidenceSource, PrivacyClass};

    fn process_evidence(pid: u32, name: &str, exe_hash: &str) -> DiscoveryEvidenceV2 {
        DiscoveryEvidenceV2 {
            evidence_id: "ev1".into(),
            source: EvidenceSource::ProcessScan,
            confidence: 1.0,
            observed_at: "2026-07-23T00:00:00Z".into(),
            privacy_class: PrivacyClass::InternalMetadata,
            redacted: true,
            data: json!({
                "process": {
                    "pid": pid,
                    "parent_pid": null,
                    "process_name": name,
                    "exe_path_hash": exe_hash,
                    "exe_path_redacted": "/usr/bin/…",
                    "cmd_template": [],
                    "cwd_hash": null,
                    "started_at_unix": null
                }
            }),
            merge_key: None,
            source_path_hash: None,
            source_path_redacted: None,
        }
    }

    #[test]
    fn extracts_process_identity_from_process_scan_evidence() {
        let evidence = vec![process_evidence(4242, "claude", "hashA")];
        let (pids, exe_hash, names) = process_identity_from_evidence(&evidence);
        assert_eq!(pids, vec![4242]);
        assert_eq!(exe_hash.as_deref(), Some("hashA"));
        assert_eq!(names, vec!["claude".to_string()]);
    }

    #[test]
    fn ignores_non_process_scan_evidence() {
        let mut ev = process_evidence(1, "x", "h");
        ev.source = EvidenceSource::McpConfig;
        let (pids, exe_hash, names) = process_identity_from_evidence(&[ev]);
        assert!(pids.is_empty());
        assert!(exe_hash.is_none());
        assert!(names.is_empty());
    }

    #[test]
    fn evidence_without_process_identity_yields_no_binding() {
        let (pids, exe_hash, names) = process_identity_from_evidence(&[]);
        assert!(pids.is_empty() && exe_hash.is_none() && names.is_empty());
    }
}
