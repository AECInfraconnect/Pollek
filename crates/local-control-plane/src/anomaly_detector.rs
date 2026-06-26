use crate::state::AppState;
use dek_domain_schema::ControlMode;
use serde_json::json;
use tokio::time::{sleep, Duration};
use tracing::{info, warn};

pub async fn start_anomaly_detector(state: AppState) {
    info!("Starting P2 Anomaly Detector...");
    loop {
        sleep(Duration::from_secs(30)).await;

        let tenant_id = "local"; // local tenant

        // 1. Fetch recent telemetry (e.g. decision logs and resource accesses)
        let evs = match state
            .telemetry_store
            .list_telemetry(tenant_id, "decision")
            .await
        {
            Ok(v) => v,
            Err(_) => continue,
        };

        // 2. Compute failure rates per agent
        let mut fail_counts: std::collections::HashMap<String, i32> =
            std::collections::HashMap::new();

        for ev in evs {
            if let (Some(agent_id), Some(allow)) = (
                ev.get("subject")
                    .and_then(|s| s.get("id"))
                    .and_then(|id| id.as_str()),
                ev.get("allow").and_then(|a| a.as_bool()),
            ) {
                if !allow {
                    *fail_counts.entry(agent_id.to_string()).or_insert(0) += 1;
                }
            }
        }

        // 3. Update trust score and trigger playbooks
        if let Ok(agents) = state.registry_store.list_agents(tenant_id).await {
            for agent in agents {
                let fails = fail_counts.get(&agent.agent_id).copied().unwrap_or(0);

                if fails > 5 {
                    warn!(
                        "AnomalyDetector: Agent {} has high failure rate ({} failures).",
                        agent.agent_id, fails
                    );

                    // Auto-remediation playbook
                    warn!("AnomalyDetector: Triggering Auto-Kill Switch playbook for agent {}! Failure rate critically high.", agent.agent_id);

                    // Emulate playbook mutating the agent's policy deployment to StrictDeny
                    if let Ok(deployments) =
                        state.policy_store.list_preset_deployments(tenant_id).await
                    {
                        for dep_val in deployments {
                            if let Ok(mut dep) = serde_json::from_value::<
                                dek_domain_schema::policy_deployment::PolicyDeployment,
                            >(dep_val)
                            {
                                if dep
                                    .control_bindings
                                    .iter()
                                    .any(|b| b.agent_id == agent.agent_id)
                                {
                                    dep.control_mode = ControlMode::StrictDeny;
                                    if let Ok(val) = serde_json::to_value(&dep) {
                                        let _ = state
                                            .policy_store
                                            .upsert_preset_deployment(
                                                tenant_id,
                                                &dep.deployment_id,
                                                &val,
                                            )
                                            .await;
                                        info!(
                                            "AnomalyDetector: Mutated deployment {} to StrictDeny.",
                                            dep.deployment_id
                                        );
                                    }
                                }
                            }
                        }
                    }

                    // Emit security event telemetry
                    let sec_event = json!({
                        "type": "security_event",
                        "schema_version": "pollek.telemetry.v2",
                        "tenant_id": tenant_id,
                        "event_id": uuid::Uuid::new_v4().to_string(),
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                        "severity": "critical",
                        "name": "auto_kill_switch_triggered",
                        "agent_id": agent.agent_id,
                        "trigger": format!("Failed {} policies in short time window", fails)
                    });
                    let event_id = sec_event["event_id"].as_str().unwrap_or("").to_string();
                    let _ = state
                        .telemetry_store
                        .put_telemetry(tenant_id, "security_event", &event_id, &sec_event)
                        .await;
                }
            }
        }
    }
}
