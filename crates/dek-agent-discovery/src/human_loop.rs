// SPDX-License-Identifier: Apache-2.0
use crate::model::*;
use dek_fingerprint_defs::model::{AgentSignatureV2, DetectionLogic, SignatureMeta};

/// คำยืนยันจาก admin ผ่าน UI (Register & Enforce / Identify).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct IdentityConfirmation {
    pub candidate_id: String,
    pub confirmed_signature_id: Option<String>, // ยืนยัน guess เดิม
    pub custom_display_name: Option<String>,    // หรือ label เอง
    pub custom_vendor: Option<String>,
    pub custom_product: Option<String>,
    pub confirmed_agent_type: InferredAgentType,
    pub confirmed_capability_tags: Vec<String>,
    pub make_local_signature: bool, // เรียนรู้ไว้ใช้ครั้งหน้า
    pub confirmed_by: String,       // audit
}

/// แปลงคำยืนยันเป็น evidence (เข้า audit log) + learned signature (ออปชัน).
pub fn apply_confirmation(
    cand: &mut DiscoveredAgentCandidateV2,
    conf: &IdentityConfirmation,
) -> Option<AgentSignatureV2> {
    // 1) บันทึกเป็น UserConfirmation evidence (confidence สูงสุด, ขึ้น audit)
    cand.evidence.push(DiscoveryEvidenceV2 {
        evidence_id: uuid::Uuid::new_v4().to_string(),
        source: EvidenceSource::UserConfirmation,
        confidence: 1.0,
        observed_at: chrono::Utc::now().to_rfc3339(),
        privacy_class: PrivacyClass::InternalMetadata,
        redacted: true,
        data: serde_json::json!({
            "confirmed_by": conf.confirmed_by,
            "signature_id": conf.confirmed_signature_id,
            "display_name": conf.custom_display_name,
        }),
        merge_key: Some(format!("user-confirm:{}", cand.candidate_id)),
        source_path_hash: None,
        source_path_redacted: None,
    });

    // 2) อัปเดตตัวตน + เลื่อนสถานะ
    if let Some(name) = &conf.custom_display_name {
        cand.display_name = name.clone();
        cand.suggested_registration.name = name.clone();
    }
    cand.vendor = conf.custom_vendor.clone().or(cand.vendor.take());
    cand.product = conf.custom_product.clone().or(cand.product.take());
    cand.inferred_agent_type = conf.confirmed_agent_type.clone();
    cand.suggested_registration.agent_type = format!("{:?}", cand.inferred_agent_type);
    cand.capability_tags = conf.confirmed_capability_tags.clone();
    cand.capability_tags.sort();
    cand.capability_tags.dedup();
    cand.labels.retain(|k, _| !k.starts_with("capability:"));
    for tag in &cand.capability_tags {
        cand.labels
            .insert(format!("capability:{tag}"), "true".into());
    }
    cand.confidence = 1.0;
    cand.status = DiscoveryStatus::Registered;
    cand.labels
        .insert("identity.confirmed".into(), "true".into());

    // 3) ออปชัน: สังเคราะห์ learned signature จาก evidence ที่ "เสถียร"
    if conf.make_local_signature {
        return Some(synthesize_local_signature(cand, conf));
    }
    None
}

fn synthesize_local_signature(
    cand: &DiscoveredAgentCandidateV2,
    conf: &IdentityConfirmation,
) -> AgentSignatureV2 {
    let mut exe_patterns = Vec::new();
    let mut binary_hashes = Vec::new();

    // Extract stable evidence from cand.evidence
    for ev in &cand.evidence {
        if ev.source == EvidenceSource::ProcessScan {
            if let Some(exe) = ev.data.get("exe_path_redacted").and_then(|v| v.as_str()) {
                if !exe.is_empty() {
                    exe_patterns.push(exe.to_string());
                }
            }
            if let Some(hash) = ev.data.get("exe_path_hash").and_then(|v| v.as_str()) {
                if !hash.is_empty() {
                    binary_hashes.push(hash.to_string());
                }
            }
        }
    }

    let ts = chrono::Utc::now().format("%Y%m%d%H%M%S").to_string();
    AgentSignatureV2 {
        id: format!(
            "local-learned-{}",
            uuid::Uuid::new_v4().to_string().replace("-", "")
        ),
        display_name: conf
            .custom_display_name
            .clone()
            .unwrap_or_else(|| cand.display_name.clone()),
        agent_type: "automation_agent".to_string(), // map properly if needed
        revision: 1,
        meta: SignatureMeta {
            author: conf.confirmed_by.clone(),
            description: "Locally learned signature from user confirmation".to_string(),
            references: vec![],
            added_in: ts,
            tags: vec!["learned".into(), "local".into()],
        },
        process_names: vec![],
        binary_hashes,
        config_paths: std::collections::BTreeMap::new(),
        config_parsers: vec![],
        ports: vec![],
        port_probe: None,
        detection_logic: DetectionLogic::AnyOf,
        control_strategies: vec![],
        risk_weight: 0.5,

        cmd_patterns: vec![],
        exe_path_patterns: exe_patterns,
        install_markers: vec![],
        cli_binaries: vec![],
        package_markers: vec![],
        env_markers: vec![],
        egress_hosts: vec![],
        vendor: conf.custom_vendor.clone(),
        product: conf.custom_product.clone(),
        capability_tags: conf.confirmed_capability_tags.clone(),
        signal_weights: None,
    }
}
