use crate::model::*;
use anyhow::Result;
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
        if wants_source("web_ai") {
            job.sources.push("web_ai".into());
        }
        if wants_source("python_framework") {
            job.sources.push("python_framework".into());
        }

        let sni_src = self.sni_source.clone();

        let (ev_tx, mut ev_rx) = tokio::sync::mpsc::channel(100);

        let mut tasks = Vec::new();

        if wants_source("process") {
            let tx_cl = ev_tx.clone();
            let config = self.config.clone();
            tasks.push(tokio::spawn(async move {
                let mut ev = Vec::new();
                if let Ok(Ok(processes)) = tokio::time::timeout(
                    std::time::Duration::from_secs(config.source_timeout_secs),
                    tokio::task::spawn_blocking(crate::process_scan::scan_processes)
                )
                .await
                .unwrap_or(Ok(Ok(vec![])))
                {
                    for p in processes {
                        let conf = crate::fingerprint::fingerprint_process(&p.process_name);
                        if conf > config.min_fingerprint_confidence {
                            ev.push(DiscoveryEvidenceV2 {
                                evidence_id: uuid::Uuid::new_v4().to_string(),
                                source: EvidenceSource::ProcessScan,
                                confidence: conf,
                                observed_at: chrono::Utc::now().to_rfc3339(),
                                privacy_class: PrivacyClass::InternalMetadata,
                                redacted: true,
                                data: serde_json::to_value(&p).unwrap_or_else(|e| {
                                    tracing::error!("Failed to serialize process scan data: {}", e);
                                    metrics::counter!("pollek_discovery_serialize_drop_total", "source" => "process_scan").increment(1);
                                    serde_json::json!({})
                                }),
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
                let _ = tx_cl.send(ev).await;
            }));
        }

        if wants_source("mcp_config") {
            let tx_cl = ev_tx.clone();
            let config = self.config.clone();
            tasks.push(tokio::spawn(async move {
                let mut ev = Vec::new();
                if let Ok(Ok(mut x)) = tokio::time::timeout(
                    std::time::Duration::from_secs(config.source_timeout_secs),
                    tokio::task::spawn_blocking(crate::mcp_scan::scan_mcp_configs)
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
            let config = self.config.clone();
            tasks.push(tokio::spawn(async move {
                let mut ev = Vec::new();
                if let Ok(Ok(mut x)) = tokio::time::timeout(
                    std::time::Duration::from_secs(config.source_timeout_secs),
                    crate::local_model_probe::probe_local_models()
                ).await {
                    ev.append(&mut x);
                }
                let _ = tx_cl.send(ev).await;
            }));
        }

        if wants_source("ide_extension") {
            let tx_cl = ev_tx.clone();
            let config = self.config.clone();
            tasks.push(tokio::spawn(async move {
                let mut ev = Vec::new();
                if let Ok(Ok(mut x)) = tokio::time::timeout(
                    std::time::Duration::from_secs(config.source_timeout_secs),
                    tokio::task::spawn_blocking(crate::ide_extension_scan::scan_ide_extensions)
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
            let config = self.config.clone();
            tasks.push(tokio::spawn(async move {
                let mut ev = Vec::new();
                if let Ok(Ok(mut x)) = tokio::time::timeout(
                    std::time::Duration::from_secs(config.source_timeout_secs),
                    tokio::task::spawn_blocking(crate::cli_agent_scan::scan_cli_agents)
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
            let config = self.config.clone();
            tasks.push(tokio::spawn(async move {
                let mut ev = Vec::new();
                if let Ok(Ok(mut x)) = tokio::time::timeout(
                    std::time::Duration::from_secs(config.source_timeout_secs),
                    tokio::task::spawn_blocking(crate::container_scan::scan_containers)
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
            let config = self.config.clone();
            tasks.push(tokio::spawn(async move {
                let mut ev = Vec::new();
                if let Ok(Ok(mut x)) = tokio::time::timeout(
                    std::time::Duration::from_secs(config.source_timeout_secs),
                    tokio::task::spawn_blocking(crate::browser_scan::scan_browsers)
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
            let config = self.config.clone();
            let timeout_secs = config.source_timeout_secs;
            tasks.push(tokio::spawn(async move {
                let mut ev = Vec::new();
                if let Ok(Ok(mut x)) = tokio::time::timeout(
                    std::time::Duration::from_secs(timeout_secs),
                    tokio::task::spawn_blocking(move || {
                        crate::web_ai_scan::scan_web_ai(
                            sni_src.as_deref(),
                            &config,
                            &defs.web_ai_signatures,
                        )
                    })
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
            let config = self.config.clone();
            tasks.push(tokio::spawn(async move {
                let mut ev = Vec::new();
                if let Ok(Ok(mut x)) = tokio::time::timeout(
                    std::time::Duration::from_secs(config.source_timeout_secs),
                    tokio::task::spawn_blocking(move || {
                        crate::installed_app_scan::scan_installed_apps(&defs.installed_app_signatures)
                    })
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
            let config = self.config.clone();
            tasks.push(tokio::spawn(async move {
                let mut ev = Vec::new();
                if let Ok(Ok(mut x)) = tokio::time::timeout(
                    std::time::Duration::from_secs(config.source_timeout_secs),
                    tokio::task::spawn_blocking(
                        crate::python_framework_scan::scan_python_frameworks,
                    )
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
        let mut sent_candidates = std::collections::HashSet::new();
        let mut final_candidates = Vec::new();

        let tenant_id = self.tenant_id.clone();

        let rx_loop = async move {
            while let Some(mut evs) = ev_rx.recv().await {
                all_evidence.append(&mut evs);
                let candidates =
                    crate::aggregator::aggregate_evidence(&tenant_id, all_evidence.clone());

                if let Some(sender) = &tx {
                    for cand in &candidates {
                        if !sent_candidates.contains(&cand.candidate_id) {
                            let _ = sender.send(cand.clone()).await;
                            sent_candidates.insert(cand.candidate_id.clone());
                        }
                    }
                }
                final_candidates = candidates;
            }
            (all_evidence, final_candidates)
        };

        // Combine task completion with deadline
        let join_all = futures::future::join_all(tasks);

        let scan_result = timeout(Duration::from_secs(15), join_all).await.is_ok();

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
