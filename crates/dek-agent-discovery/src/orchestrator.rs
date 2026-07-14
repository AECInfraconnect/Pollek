use crate::model::*;
use anyhow::Result;
use sha2::{Digest, Sha256};
use tokio::time::{timeout, Duration};

pub struct DiscoveryOrchestrator {
    tenant_id: String,
    sni_source: Option<std::sync::Arc<dyn crate::web_ai_scan::SniFlowSource>>,
    definitions: std::sync::Arc<dek_fingerprint_defs::model::FingerprintDefinition>,
    config: crate::config::DiscoveryConfig,
}

impl DiscoveryOrchestrator {
    pub fn new(
        tenant_id: &str,
        definitions: std::sync::Arc<dek_fingerprint_defs::model::FingerprintDefinition>,
    ) -> Self {
        Self {
            tenant_id: tenant_id.to_string(),
            sni_source: None,
            definitions,
            config: crate::config::DiscoveryConfig::default(),
        }
    }

    pub fn with_sni_source(
        mut self,
        source: std::sync::Arc<dyn crate::web_ai_scan::SniFlowSource>,
    ) -> Self {
        self.sni_source = Some(source);
        self
    }

    pub async fn run_scan(
        &self,
        scan_id: &str,
        req: &serde_json::Value,
        tx: Option<tokio::sync::mpsc::Sender<DiscoveredAgentCandidateV2>>,
    ) -> Result<(DiscoveryScanJob, Vec<DiscoveredAgentCandidateV2>)> {
        let scan_config = effective_config(&self.config, req);
        let mut job = DiscoveryScanJob {
            scan_id: scan_id.to_string(),
            tenant_id: self.tenant_id.clone(),
            status: ScanJobStatus::Running,
            started_at: Some(chrono::Utc::now().to_rfc3339()),
            finished_at: None,
            sources: Vec::new(),
            error: None,
            candidates_found: 0,
        };

        let sources_req = req.get("sources").and_then(|s| s.as_array());
        let wants_source =
            |s: &str| sources_req.is_none_or(|a| a.iter().any(|v| v.as_str() == Some(s)));

        if wants_source("process") {
            job.sources.push("process".into());
        }
        if wants_source("mcp_config") {
            job.sources.push("mcp_config".into());
        }
        if wants_source("local_model") {
            job.sources.push("local_model".into());
        }
        if wants_source("ide_extension") {
            job.sources.push("ide_extension".into());
        }
        if wants_source("cli_agent") {
            job.sources.push("cli_agent".into());
        }
        if wants_source("container") {
            job.sources.push("container".into());
        }
        if wants_source("browser_extension") {
            job.sources.push("browser_extension".into());
        }
        if wants_source("installed_app") {
            job.sources.push("installed_app".into());
        }
        if wants_source("web_ai") {
            job.sources.push("web_ai".into());
            job.sources.push("browser_window".into());
        }
        if wants_source("python_framework") {
            job.sources.push("python_framework".into());
        }

        let sni_src = self.sni_source.clone();

        let (ev_tx, mut ev_rx) = tokio::sync::mpsc::channel(100);

        let mut tasks = Vec::new();

        if wants_source("process") {
            let tx_cl = ev_tx.clone();
            let config = scan_config.clone();
            let defs = self.definitions.clone();
            tasks.push(tokio::spawn(async move {
                let mut ev = Vec::new();
                if let Ok(Ok(processes)) = tokio::time::timeout(
                    std::time::Duration::from_secs(config.source_timeout_secs),
                    tokio::task::spawn_blocking(crate::process_scan::scan_processes),
                )
                .await
                .unwrap_or(Ok(Ok(vec![])))
                {
                    for p in processes {
                        if crate::browser_window_scan::is_browser_process(
                            &p.process_name,
                            &defs.browser_processes,
                        ) {
                            continue;
                        }
                        let cmdline = p.cmd_template.join(" ");
                        let facts = crate::fingerprint::ProcessFacts {
                            process_name: &p.process_name,
                            exe_path: p.exe_path_redacted.as_deref().unwrap_or(""),
                            cmdline: &cmdline,
                        };
                        let resolved = crate::fingerprint::fingerprint_process_v2_with_hints(
                            &facts,
                            &defs.signatures,
                            &defs.installed_app_signatures,
                            Some(&defs.ai_process_hints),
                        );

                        let above = resolved.confidence >= config.min_fingerprint_confidence;
                        if above || resolved.confidence >= config.min_unconfirmed_confidence {
                            ev.push(DiscoveryEvidenceV2 {
                                evidence_id: uuid::Uuid::new_v4().to_string(),
                                source: EvidenceSource::ProcessScan,
                                confidence: resolved.confidence,
                                observed_at: chrono::Utc::now().to_rfc3339(),
                                privacy_class: PrivacyClass::InternalMetadata,
                                redacted: true,
                                data: serde_json::json!({
                                    "process": p,
                                    "resolved_name": resolved.display_name,
                                    "vendor": resolved.vendor,
                                    "matched_signature_id": resolved.matched_signature_id,
                                    "confirmed": above,
                                }),
                                merge_key: resolved.matched_signature_id.clone().or_else(|| {
                                    Some(format!("{:?}:{}", p.exe_path_hash, p.process_name))
                                }),
                                source_path_hash: p.exe_path_hash.clone(),
                                source_path_redacted: Some(p.process_name.clone()),
                            });
                        }
                    }
                }
                let _ = tx_cl.send(ev).await;
            }));
        }

        if wants_source("mcp_config") {
            let tx_cl = ev_tx.clone();
            let config = scan_config.clone();
            tasks.push(tokio::spawn(async move {
                let mut ev = Vec::new();
                if let Ok(Ok(mut x)) = tokio::time::timeout(
                    std::time::Duration::from_secs(config.source_timeout_secs),
                    tokio::task::spawn_blocking(crate::mcp_scan::scan_mcp_configs),
                )
                .await
                .unwrap_or(Ok(Ok(vec![])))
                {
                    ev.append(&mut x);
                }
                let _ = tx_cl.send(ev).await;
            }));
        }

        if wants_source("local_model") {
            let tx_cl = ev_tx.clone();
            let config = scan_config.clone();
            tasks.push(tokio::spawn(async move {
                let mut ev = Vec::new();
                if let Ok(Ok(mut x)) = tokio::time::timeout(
                    std::time::Duration::from_secs(config.source_timeout_secs),
                    crate::local_model_probe::probe_local_models(),
                )
                .await
                {
                    ev.append(&mut x);
                }
                let _ = tx_cl.send(ev).await;
            }));
        }

        if wants_source("ide_extension") {
            let tx_cl = ev_tx.clone();
            let config = scan_config.clone();
            tasks.push(tokio::spawn(async move {
                let mut ev = Vec::new();
                if let Ok(Ok(mut x)) = tokio::time::timeout(
                    std::time::Duration::from_secs(config.source_timeout_secs),
                    tokio::task::spawn_blocking(crate::ide_extension_scan::scan_ide_extensions),
                )
                .await
                .unwrap_or(Ok(Ok(vec![])))
                {
                    ev.append(&mut x);
                }
                let _ = tx_cl.send(ev).await;
            }));
        }

        if wants_source("cli_agent") {
            let tx_cl = ev_tx.clone();
            let config = scan_config.clone();
            tasks.push(tokio::spawn(async move {
                let mut ev = Vec::new();
                if let Ok(Ok(mut x)) = tokio::time::timeout(
                    std::time::Duration::from_secs(config.source_timeout_secs),
                    tokio::task::spawn_blocking(crate::cli_agent_scan::scan_cli_agents),
                )
                .await
                .unwrap_or(Ok(Ok(vec![])))
                {
                    ev.append(&mut x);
                }
                let _ = tx_cl.send(ev).await;
            }));
        }

        if wants_source("container") {
            let tx_cl = ev_tx.clone();
            let config = scan_config.clone();
            tasks.push(tokio::spawn(async move {
                let mut ev = Vec::new();
                if let Ok(Ok(mut x)) = tokio::time::timeout(
                    std::time::Duration::from_secs(config.source_timeout_secs),
                    tokio::task::spawn_blocking(crate::container_scan::scan_containers),
                )
                .await
                .unwrap_or(Ok(Ok(vec![])))
                {
                    ev.append(&mut x);
                }
                let _ = tx_cl.send(ev).await;
            }));
        }

        if wants_source("browser_extension") {
            let tx_cl = ev_tx.clone();
            let config = scan_config.clone();
            tasks.push(tokio::spawn(async move {
                let mut ev = Vec::new();
                if let Ok(Ok(mut x)) = tokio::time::timeout(
                    std::time::Duration::from_secs(config.source_timeout_secs),
                    tokio::task::spawn_blocking(crate::browser_scan::scan_browsers),
                )
                .await
                .unwrap_or(Ok(Ok(vec![])))
                {
                    ev.append(&mut x);
                }
                let _ = tx_cl.send(ev).await;
            }));
        }

        if wants_source("web_ai") {
            let tx_cl = ev_tx.clone();
            let defs = self.definitions.clone();
            let config = scan_config.clone();
            let timeout_secs = config.source_timeout_secs;
            tasks.push(tokio::spawn(async move {
                let mut ev = Vec::new();
                if let Ok(Ok(mut x)) = tokio::time::timeout(
                    std::time::Duration::from_secs(timeout_secs),
                    tokio::task::spawn_blocking(move || {
                        let mut evidence = crate::web_ai_scan::scan_web_ai(
                            sni_src.as_deref(),
                            &config,
                            &defs.web_ai_signatures,
                        )?;
                        let mut window_evidence = crate::browser_window_scan::scan_browser_windows(
                            &defs.web_ai_signatures,
                            &defs.browser_processes,
                        )?;
                        evidence.append(&mut window_evidence);
                        Ok::<_, anyhow::Error>(evidence)
                    }),
                )
                .await
                .unwrap_or(Ok(Ok(vec![])))
                {
                    ev.append(&mut x);
                }
                let _ = tx_cl.send(ev).await;
            }));
        }

        if wants_source("installed_app") {
            let tx_cl = ev_tx.clone();
            let defs = self.definitions.clone();
            let config = scan_config.clone();
            tasks.push(tokio::spawn(async move {
                let mut ev = Vec::new();
                if let Ok(Ok(mut x)) = tokio::time::timeout(
                    std::time::Duration::from_secs(config.source_timeout_secs),
                    tokio::task::spawn_blocking(move || {
                        crate::installed_app_scan::scan_installed_apps(
                            &defs.installed_app_signatures,
                        )
                    }),
                )
                .await
                .unwrap_or(Ok(Ok(vec![])))
                {
                    ev.append(&mut x);
                }
                let _ = tx_cl.send(ev).await;
            }));
        }

        if wants_source("python_framework") {
            let tx_cl = ev_tx.clone();
            let config = scan_config.clone();
            tasks.push(tokio::spawn(async move {
                let mut ev = Vec::new();
                if let Ok(Ok(mut x)) = tokio::time::timeout(
                    std::time::Duration::from_secs(config.source_timeout_secs),
                    tokio::task::spawn_blocking(
                        crate::python_framework_scan::scan_python_frameworks,
                    ),
                )
                .await
                {
                    ev.append(&mut x);
                }
                let _ = tx_cl.send(ev).await;
            }));
        }

        // drop original tx so the rx loop will eventually terminate when all tasks complete
        drop(ev_tx);

        let mut all_evidence = Vec::new();
        let mut sent_candidate_digests = std::collections::HashMap::new();
        let mut final_candidates = Vec::new();

        let tenant_id = self.tenant_id.clone();
        let device_id = "device-local".to_string();
        let scan_id_for_candidates = scan_id.to_string();

        let rx_loop = async move {
            while let Some(mut evs) = ev_rx.recv().await {
                all_evidence.append(&mut evs);
                let mut candidates = crate::aggregator::aggregate_evidence(
                    &tenant_id,
                    &device_id,
                    all_evidence.clone(),
                );
                for cand in &mut candidates {
                    tag_candidate_with_scan(cand, &scan_id_for_candidates);
                }

                if let Some(sender) = &tx {
                    for cand in &candidates {
                        let digest = candidate_snapshot_digest(cand);
                        let previous = sent_candidate_digests.get(&cand.candidate_id);
                        if previous != Some(&digest) {
                            let _ = sender.send(cand.clone()).await;
                            sent_candidate_digests.insert(cand.candidate_id.clone(), digest);
                        }
                    }
                }
                final_candidates = candidates;
            }
            (all_evidence, final_candidates)
        };

        // Combine task completion with deadline
        let join_all = futures::future::join_all(tasks);

        let scan_result = timeout(
            Duration::from_secs(scan_config.total_deadline_secs),
            join_all,
        )
        .await
        .is_ok();

        let (_all_evidence, candidates) = rx_loop.await;

        if !scan_result {
            job.error = Some("scan exceeded deadline; returning partial results".into());
            job.status = ScanJobStatus::Partial;
        }

        if job.status != ScanJobStatus::Failed && job.status != ScanJobStatus::Partial {
            job.status = ScanJobStatus::Completed;
        }
        job.finished_at = Some(chrono::Utc::now().to_rfc3339());
        job.candidates_found = candidates.len() as u32;

        Ok((job, candidates))
    }
}

fn tag_candidate_with_scan(candidate: &mut DiscoveredAgentCandidateV2, scan_id: &str) {
    candidate.last_scan_id = Some(scan_id.to_string());
    if !candidate.scan_ids.iter().any(|id| id == scan_id) {
        candidate.scan_ids.push(scan_id.to_string());
    }

    for evidence in &mut candidate.evidence {
        if let Some(data) = evidence.data.as_object_mut() {
            data.insert("scan_id".to_string(), serde_json::json!(scan_id));
        }
    }
}

fn effective_config(
    base: &crate::config::DiscoveryConfig,
    req: &serde_json::Value,
) -> crate::config::DiscoveryConfig {
    let mut config = base.clone();
    let deep_scan_requested = req
        .get("scan_mode")
        .and_then(|value| value.as_str())
        .is_some_and(|mode| mode.eq_ignore_ascii_case("deep"))
        || req
            .get("deep_scan")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);

    if deep_scan_requested {
        config.source_timeout_secs = config.source_timeout_secs.max(10);
        config.total_deadline_secs = config.total_deadline_secs.max(45);
    }

    if let Some(value) = req
        .get("source_timeout_secs")
        .and_then(serde_json::Value::as_u64)
    {
        config.source_timeout_secs = value.clamp(3, 30);
    }

    if let Some(value) = req
        .get("total_deadline_secs")
        .and_then(serde_json::Value::as_u64)
    {
        config.total_deadline_secs = value.clamp(10, 120);
    }

    if config.total_deadline_secs < config.source_timeout_secs {
        config.total_deadline_secs = config.source_timeout_secs;
    }

    config
}

fn candidate_snapshot_digest(candidate: &DiscoveredAgentCandidateV2) -> String {
    let payload = serde_json::to_vec(candidate)
        .unwrap_or_else(|_| candidate.candidate_id.as_bytes().to_vec());
    let mut hasher = Sha256::new();
    hasher.update(payload);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn candidate_fixture(evidence_len: usize) -> DiscoveredAgentCandidateV2 {
        let mut evidence = Vec::new();
        for idx in 0..evidence_len {
            evidence.push(DiscoveryEvidenceV2 {
                evidence_id: format!("ev_{idx}"),
                source: EvidenceSource::ProcessScan,
                confidence: 0.7,
                observed_at: "2026-06-27T00:00:00Z".into(),
                privacy_class: PrivacyClass::InternalMetadata,
                redacted: true,
                data: serde_json::json!({ "idx": idx }),
                merge_key: Some("agent:test".into()),
                source_path_hash: None,
                source_path_redacted: None,
            });
        }

        DiscoveredAgentCandidateV2 {
            schema_version: "pollek.agent_discovery_candidate.v2".into(),
            candidate_id: "cand_test".into(),
            tenant_id: "local".into(),
            device_id: "device-local".into(),
            status: DiscoveryStatus::Discovered,
            canonical_service_id: "test_agent".into(),
            surface_group_id: "test_agent".into(),
            authority_boundary: AuthorityBoundary::LocalDevice,
            entity_role: EntityRole::LocalAgentHost,
            duplicate_policy: DuplicatePolicy::Standalone,
            control_parent_id: None,
            grouping_reason: None,
            observe_scope: "local_process_file_network_tool_metadata".into(),
            enforce_scope: "local_policy_pep_when_installed".into(),
            related_surfaces: vec![],
            instance_count: 1,
            matched_signature_id: Some("test_agent".into()),
            display_name: "Test Agent".into(),
            vendor: Some("Test".into()),
            product: Some("Agent".into()),
            inferred_agent_type: InferredAgentType::DesktopAgent,
            confidence: 0.7,
            risk_score: 20,
            capability_tags: vec!["llm.chat".into()],
            matched_signals: vec![],
            first_seen: "2026-06-27T00:00:00Z".into(),
            last_seen: "2026-06-27T00:00:01Z".into(),
            scan_ids: vec![],
            last_scan_id: None,
            evidence,
            discovered_configs: vec![],
            discovered_endpoints: vec![],
            discovered_mcp_servers: vec![],
            suggested_registration: SuggestedAgentRegistration {
                agent_id: "agent_test".into(),
                name: "Test Agent".into(),
                agent_type: "DesktopAgent".into(),
                runtime_name: "native".into(),
                process_path_hash: None,
                executable_signer: None,
                declared_tools: vec![],
                declared_resources: vec![],
                mcp_stdio_config_paths: vec![],
                mcp_http_urls: vec![],
                local_model_endpoints: vec![],
                browser_extension_evidence: vec![],
                trust_level: "Unknown".into(),
                initial_status: "pending_approval".into(),
            },
            suggested_observation_profile: ObservationProfile {
                mode: ObservationMode::ObserveOnly,
                collect_process_metadata: true,
                collect_network_metadata: true,
                collect_mcp_tool_metadata: false,
                collect_token_usage: false,
                collect_file_metadata: false,
                collect_raw_prompt: false,
                collect_raw_response: false,
                retention_days: 14,
            },
            observation_coverage: Vec::new(),
            suggested_control_bindings: vec![],
            telemetry_plan: TelemetryPlan {
                events_endpoint: "/v1/telemetry/events".into(),
                metrics_endpoint: "/v1/metrics".into(),
                capture_tool_calls: false,
                capture_arguments: false,
                redact_env_keys: vec![],
                risk_signals: vec![],
            },
            labels: BTreeMap::new(),
        }
    }

    #[test]
    fn deep_scan_request_extends_budgets_with_bounds() {
        let base = crate::config::DiscoveryConfig::default();
        let config = effective_config(
            &base,
            &serde_json::json!({
                "scan_mode": "deep",
                "source_timeout_secs": 60,
                "total_deadline_secs": 180
            }),
        );

        assert_eq!(config.source_timeout_secs, 30);
        assert_eq!(config.total_deadline_secs, 120);
    }

    #[test]
    fn candidate_digest_changes_when_scan_evidence_grows() {
        let first = candidate_snapshot_digest(&candidate_fixture(1));
        let second = candidate_snapshot_digest(&candidate_fixture(2));

        assert_ne!(first, second);
    }
}
