use crate::model::*;
use anyhow::Result;

pub struct DiscoveryOrchestrator {
    tenant_id: String,
}

impl DiscoveryOrchestrator {
    pub fn new(tenant_id: &str) -> Self {
        Self {
            tenant_id: tenant_id.to_string(),
        }
    }

    pub async fn run_scan(
        &self,
        req: &serde_json::Value,
    ) -> Result<(DiscoveryScanJob, Vec<DiscoveredAgentCandidateV2>)> {
        let scan_id = format!("scan_{}", uuid::Uuid::new_v4());
        let mut job = DiscoveryScanJob {
            scan_id: scan_id.clone(),
            tenant_id: self.tenant_id.clone(),
            status: ScanJobStatus::Running,
            started_at: Some(chrono::Utc::now().to_rfc3339()),
            finished_at: None,
            sources: Vec::new(),
            error: None,
            candidates_found: 0,
        };

        let mut all_evidence = Vec::new();
        let sources_req = req.get("sources").and_then(|s| s.as_array());

        let wants_source =
            |s: &str| sources_req.is_none_or(|a| a.iter().any(|v| v.as_str() == Some(s)));

        // 1. Process Scan
        if wants_source("process") {
            job.sources.push("process".into());
            match crate::process_scan::scan_processes() {
                Ok(processes) => {
                    let config = crate::config::DiscoveryConfig::default();
                    for p in processes {
                        let conf = crate::fingerprint::fingerprint_process(&p.process_name);
                        if conf > config.min_fingerprint_confidence {
                            all_evidence.push(DiscoveryEvidenceV2 {
                                evidence_id: uuid::Uuid::new_v4().to_string(),
                                source: EvidenceSource::ProcessScan,
                                confidence: conf,
                                observed_at: chrono::Utc::now().to_rfc3339(),
                                privacy_class: PrivacyClass::InternalMetadata,
                                redacted: true,
                                data: serde_json::to_value(&p).unwrap_or_default(),
                                merge_key: Some(format!(
                                    "{:?}:{}",
                                    p.exe_path_hash, p.process_name
                                )),
                                source_path_hash: p.exe_path_hash.clone(),
                                source_path_redacted: Some(p.process_name.clone()),
                            });
                        }
                    }
                }
                Err(e) => {
                    job.error = Some(e.to_string());
                    job.status = ScanJobStatus::Failed;
                }
            }
        }

        // 2. MCP Scan
        if wants_source("mcp_config") {
            job.sources.push("mcp_config".into());
            if let Ok(mut mcp_evidence) = crate::mcp_scan::scan_mcp_configs() {
                all_evidence.append(&mut mcp_evidence);
            }
        }

        // 3. Local Model Probe
        if wants_source("local_model") {
            job.sources.push("local_model".into());
            if let Ok(mut lm_evidence) = crate::local_model_probe::probe_local_models().await {
                all_evidence.append(&mut lm_evidence);
            }
        }

        // 4. IDE Extension Scan
        if wants_source("ide_extension") {
            job.sources.push("ide_extension".into());
            if let Ok(mut ide_evidence) = crate::ide_extension_scan::scan_ide_extensions() {
                all_evidence.append(&mut ide_evidence);
            }
        }

        // 5. CLI Agent Scan
        if wants_source("cli_agent") {
            job.sources.push("cli_agent".into());
            if let Ok(mut cli_evidence) = crate::cli_agent_scan::scan_cli_agents() {
                all_evidence.append(&mut cli_evidence);
            }
        }

        // 6. Container Scan (Phase 6 Optional)
        if wants_source("container") {
            job.sources.push("container".into());
            if let Ok(mut container_evidence) = crate::container_scan::scan_containers() {
                all_evidence.append(&mut container_evidence);
            }
        }

        // 7. Browser Extension Scan (Phase 6 Optional)
        if wants_source("browser_extension") {
            job.sources.push("browser_extension".into());
            if let Ok(mut browser_evidence) = crate::browser_scan::scan_browsers() {
                all_evidence.append(&mut browser_evidence);
            }
        }

        let candidates = crate::aggregator::aggregate_evidence(&self.tenant_id, all_evidence);

        if job.status != ScanJobStatus::Failed {
            job.status = ScanJobStatus::Completed;
        }
        job.finished_at = Some(chrono::Utc::now().to_rfc3339());
        job.candidates_found = candidates.len() as u32;

        Ok((job, candidates))
    }
}
