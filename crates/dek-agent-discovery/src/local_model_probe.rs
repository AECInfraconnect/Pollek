use crate::model::*;
use anyhow::Result;
use std::time::Duration;

pub async fn probe_local_models() -> Result<Vec<DiscoveryEvidenceV2>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(500))
        .build()?;

    let mut tasks = Vec::new();

    // 1. Probe Ollama
    {
        let client_cl = client.clone();
        tasks.push(tokio::spawn(async move {
            let mut local_ev = Vec::new();
            if let Ok(res) = client_cl
                .get("http://127.0.0.1:11434/api/tags")
                .send()
                .await
            {
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

                        local_ev.push(DiscoveryEvidenceV2 {
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
            local_ev
        }));
    }

    // 2. Probe OpenAI-compatible APIs (LM Studio, vLLM, Jan, GPT4All, llama.cpp, text-gen-webui)
    let openai_probes = vec![
        ("lmstudio", 1234),
        ("vllm", 8000),
        ("jan", 1337),
        ("gpt4all", 4891),
        ("llama.cpp/localai", 8080),
        ("text-gen-webui", 5000),
    ];

    for (provider, port) in openai_probes {
        let client_cl = client.clone();
        tasks.push(tokio::spawn(async move {
            let mut local_ev = Vec::new();
            let url = format!("http://127.0.0.1:{}/v1/models", port);
            if let Ok(res) = client_cl.get(&url).send().await {
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

                        local_ev.push(DiscoveryEvidenceV2 {
                            evidence_id: uuid::Uuid::new_v4().to_string(),
                            source: EvidenceSource::LocalModelServer,
                            confidence: 0.90,
                            observed_at: chrono::Utc::now().to_rfc3339(),
                            privacy_class: PrivacyClass::PublicMetadata,
                            redacted: false,
                            data: serde_json::json!({
                                "provider": provider,
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
            local_ev
        }));
    }

    // 3. Probe typical MCP ports: first attempt a bounded, read-only MCP
    // capability listing (Streamable HTTP transport); fall back to the
    // lighter SSE-detection heuristic when that doesn't yield a live
    // handshake so we still record the endpoint as a candidate.
    let mcp_ports = [3000, 3001, 8080, 8000];
    for port in mcp_ports {
        let client_cl = client.clone();
        tasks.push(tokio::spawn(async move {
            let mut local_ev = Vec::new();

            for mcp_path in ["/mcp", "/sse"] {
                let url = format!("http://127.0.0.1:{}{}", port, mcp_path);
                if let Some(snapshot) =
                    crate::capability_retrieval::probe_mcp_http_capabilities(&client_cl, &url).await
                {
                    local_ev.push(DiscoveryEvidenceV2 {
                        evidence_id: uuid::Uuid::new_v4().to_string(),
                        source: EvidenceSource::PortProbe,
                        confidence: 0.98,
                        observed_at: chrono::Utc::now().to_rfc3339(),
                        privacy_class: PrivacyClass::PublicMetadata,
                        redacted: false,
                        data: serde_json::json!({
                            "provider": "mcp_server",
                            "transport": "http",
                            "endpoint": url,
                            "mcp": {
                                "server_name": snapshot.server_name,
                                "server_version": snapshot.server_version,
                                "protocol_version": snapshot.protocol_version,
                                "tools": snapshot.tools,
                                "tools_truncated": snapshot.tools_truncated,
                                "resources": snapshot.resources,
                                "resources_truncated": snapshot.resources_truncated,
                                "prompts": snapshot.prompts,
                                "prompts_truncated": snapshot.prompts_truncated,
                            },
                        }),
                        merge_key: Some(format!("mcp_sse_{}", port)),
                        source_path_hash: None,
                        source_path_redacted: Some(url),
                    });
                    return local_ev;
                }
            }

            let url = format!("http://127.0.0.1:{}/sse", port);
            if let Ok(res) = client_cl.get(&url).send().await {
                // MCP SSE might return 405 Method Not Allowed on GET, or 200 with text/event-stream
                if res.status().is_success() || res.status().as_u16() == 405 {
                    let is_sse = res
                        .headers()
                        .get(reqwest::header::CONTENT_TYPE)
                        .and_then(|v| v.to_str().ok())
                        .map(|s| s.contains("text/event-stream"))
                        .unwrap_or(res.status().is_success());

                    if is_sse || res.status().as_u16() == 405 {
                        local_ev.push(DiscoveryEvidenceV2 {
                            evidence_id: uuid::Uuid::new_v4().to_string(),
                            source: EvidenceSource::PortProbe,
                            confidence: 0.70,
                            observed_at: chrono::Utc::now().to_rfc3339(),
                            privacy_class: PrivacyClass::PublicMetadata,
                            redacted: false,
                            data: serde_json::json!({
                                "provider": "mcp_server",
                                "transport": "sse",
                                "endpoint": url,
                            }),
                            merge_key: Some(format!("mcp_sse_{}", port)),
                            source_path_hash: None,
                            source_path_redacted: Some(url),
                        });
                    }
                }
            }
            local_ev
        }));
    }

    let mut evidence = Vec::new();
    let results = futures::future::join_all(tasks).await;
    for mut evs in results.into_iter().flatten() {
        evidence.append(&mut evs);
    }

    Ok(evidence)
}
