use crate::model::*;
use anyhow::Result;
use std::time::Duration;

pub async fn probe_local_models() -> Result<Vec<DiscoveryEvidenceV2>> {
    let mut evidence = Vec::new();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(500))
        .build()?;

    // 1. Probe Ollama
    if let Ok(res) = client.get("http://127.0.0.1:11434/api/tags").send().await {
        if res.status().is_success() {
            if let Ok(json) = res.json::<serde_json::Value>().await {
                let models = json
                    .get("models")
                    .and_then(|m| m.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| {
                                v.get("name")
                                    .and_then(|n| n.as_str())
                                    .map(|s| s.to_string())
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();

                evidence.push(DiscoveryEvidenceV2 {
                    evidence_id: uuid::Uuid::new_v4().to_string(),
                    source: EvidenceSource::LocalModelServer,
                    confidence: 0.99,
                    observed_at: chrono::Utc::now().to_rfc3339(),
                    privacy_class: PrivacyClass::PublicMetadata,
                    redacted: false,
                    data: serde_json::json!({
                        "provider": "ollama",
                        "endpoint": "http://127.0.0.1:11434",
                        "models": models,
                    }),
                    merge_key: Some("http://127.0.0.1:11434".into()),
                    source_path_hash: None,
                    source_path_redacted: Some("http://127.0.0.1:11434".into()),
                });
            }
        }
    }

    // 2. Probe OpenAI-compatible (LM Studio / vLLM default ports: 1234, 8000)
    let openai_ports = [1234, 8000];
    for port in openai_ports {
        let url = format!("http://127.0.0.1:{}/v1/models", port);
        if let Ok(res) = client.get(&url).send().await {
            if res.status().is_success() {
                if let Ok(json) = res.json::<serde_json::Value>().await {
                    let models = json
                        .get("data")
                        .and_then(|d| d.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| {
                                    v.get("id").and_then(|n| n.as_str()).map(|s| s.to_string())
                                })
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();

                    evidence.push(DiscoveryEvidenceV2 {
                        evidence_id: uuid::Uuid::new_v4().to_string(),
                        source: EvidenceSource::LocalModelServer,
                        confidence: 0.90,
                        observed_at: chrono::Utc::now().to_rfc3339(),
                        privacy_class: PrivacyClass::PublicMetadata,
                        redacted: false,
                        data: serde_json::json!({
                            "provider": "openai_compatible",
                            "endpoint": format!("http://127.0.0.1:{}", port),
                            "models": models,
                        }),
                        merge_key: Some(format!("http://127.0.0.1:{}", port)),
                        source_path_hash: None,
                        source_path_redacted: Some(format!("http://127.0.0.1:{}", port)),
                    });
                }
            }
        }
    }

    Ok(evidence)
}
