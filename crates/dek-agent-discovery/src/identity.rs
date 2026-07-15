// SPDX-License-Identifier: Apache-2.0

use dek_fingerprint_defs::model::AgentSignatureV2;

/// สัญญาณที่ดึงได้จาก process หนึ่ง (รวมจากหลาย scanner) ก่อนเอาไป match กับ signature DB.
#[derive(Debug, Default, Clone)]
pub struct ResolutionContext {
    pub process_name: String,
    pub cmd_redacted: String,          // argv join + redact แล้ว (§6)
    pub exe_path_norm: Option<String>, // normalize ${HOME}
    pub cwd_norm: Option<String>,
    pub binary_hash: Option<String>, // ถ้าคำนวณได้
    pub listening_ports: Vec<u16>,
    pub present_paths: Vec<String>, // path ที่ตรวจแล้วว่า exists (install/config markers)
    pub cli_on_path: Vec<String>,   // binary ที่เจอบน PATH
    pub packages: Vec<(String, String)>, // (ecosystem, name) ที่ติดตั้ง
    pub egress_hosts: Vec<String>,  // จาก web_ai_scan / sni_source
    pub env_present: Vec<String>,   // ชื่อ ENV ที่มี (ไม่เก็บค่า)
}

/// ผลการ match หนึ่ง signature ต่อหนึ่ง process — มี breakdown เพื่อ explainability + HITL.
#[derive(Debug, Clone)]
pub struct AgentMatch {
    pub signature_id: String,
    pub display_name: String,
    pub vendor: Option<String>,
    pub product: Option<String>,
    pub agent_type: String,
    pub confidence: f64,
    pub matched_signals: Vec<MatchedSignal>, // อธิบายว่า "รู้ได้เพราะอะไร"
    pub capability_tags: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MatchedSignal {
    pub kind: String,   // "install_marker" | "cmd_pattern" | ...
    pub detail: String, // เช่น "~/.openclaw/openclaw.json exists" (redacted)
    pub weight: f64,
}

pub struct ResolverDecision {
    pub best: Option<AgentMatch>,
    pub runner_up: Option<AgentMatch>,
    pub needs_human: bool, // true เมื่อ confidence ต่ำ หรือ best≈runner_up (กำกวม)
    pub reason: String,
}

pub fn score_signature(ctx: &ResolutionContext, sig: &AgentSignatureV2) -> Option<AgentMatch> {
    let w = sig.signal_weights.clone().unwrap_or_default();
    let mut signals: Vec<MatchedSignal> = Vec::new();
    let mut score = 0.0_f64;

    // --- สัญญาณอ่อน: ชื่อ process ---
    // ชื่อ interpreter กลาง (node/python/bun/…) ไม่นับเป็นสัญญาณ: agent คนละตัว
    // จำนวนมากรันใต้ interpreter เดียวกัน จะกลายเป็น false positive/candidate ซ้ำ
    if !ctx.process_name.trim().is_empty()
        && !crate::signature_match::is_generic_host_process(&ctx.process_name)
        && sig.process_names.iter().any(|n| {
            !n.trim().is_empty()
                && !crate::signature_match::is_generic_host_process(n)
                && n.eq_ignore_ascii_case(&ctx.process_name)
        })
    {
        push(
            &mut signals,
            &mut score,
            "process_name",
            &ctx.process_name,
            w.process_name,
        );
    }
    // --- สัญญาณแข็ง: install marker (exists) ---
    for m in &sig.install_markers {
        if m.path.trim().is_empty() {
            continue;
        }
        let expanded = expand_path(&m.path);
        if ctx.present_paths.iter().any(|p| p == &expanded) {
            push(
                &mut signals,
                &mut score,
                "install_marker",
                &crate::redaction::redact_path_for_ui(&m.path),
                m.weight.unwrap_or(w.install_marker),
            );
        }
    }
    // --- cmd pattern (argv ที่ redact แล้ว) ---
    for pat in &sig.cmd_patterns {
        if !pat.trim().is_empty()
            && !ctx.cmd_redacted.trim().is_empty()
            && regex_match(pat, &ctx.cmd_redacted)
        {
            push(&mut signals, &mut score, "cmd_pattern", pat, w.cmd_pattern);
        }
    }
    // --- exe path glob ---
    if let Some(exe) = &ctx.exe_path_norm {
        for pat in &sig.exe_path_patterns {
            if !pat.trim().is_empty() && glob_match(pat, exe) {
                push(&mut signals, &mut score, "exe_path", pat, w.exe_path);
            }
        }
    }
    // --- CLI binary บน PATH ---
    for cli in &sig.cli_binaries {
        if !cli.trim().is_empty() && ctx.cli_on_path.iter().any(|c| c == cli) {
            push(&mut signals, &mut score, "cli_binary", cli, w.cli_binary);
        }
    }
    // --- package marker ---
    for pkg in &sig.package_markers {
        if !pkg.ecosystem.trim().is_empty()
            && !pkg.name.trim().is_empty()
            && ctx
                .packages
                .iter()
                .any(|(eco, name)| eco == &pkg.ecosystem && name == &pkg.name)
        {
            push(&mut signals, &mut score, "package", &pkg.name, w.package);
        }
    }
    // --- config path ---
    for (name, paths) in &sig.config_paths {
        for pat in paths {
            if !pat.trim().is_empty() && ctx.present_paths.iter().any(|p| glob_match(pat, p)) {
                push(&mut signals, &mut score, "config_path", name, w.config_path);
            }
        }
    }

    if let Some(h) = &ctx.binary_hash {
        if !h.trim().is_empty() && sig.binary_hashes.iter().any(|bh| bh == h) {
            push(
                &mut signals,
                &mut score,
                "binary_hash",
                "exact",
                w.binary_hash,
            );
        }
    }
    for host in &sig.egress_hosts {
        if !host.trim().is_empty() && ctx.egress_hosts.iter().any(|h| h == host) {
            push(&mut signals, &mut score, "egress", host, w.egress);
        }
    }
    for p in &sig.ports {
        if *p != 0 && ctx.listening_ports.contains(p) {
            push(&mut signals, &mut score, "port", &p.to_string(), w.port);
        }
    }

    if signals.is_empty() {
        return None;
    }

    Some(AgentMatch {
        signature_id: sig.id.clone(),
        display_name: sig.display_name.clone(),
        vendor: sig.vendor.clone(),
        product: sig.product.clone(),
        agent_type: sig.agent_type.clone(),
        confidence: score.min(1.0),
        matched_signals: signals,
        capability_tags: sig.capability_tags.clone(),
    })
}

fn push(v: &mut Vec<MatchedSignal>, score: &mut f64, kind: &str, detail: &str, w: f64) {
    *score += w;
    v.push(MatchedSignal {
        kind: kind.into(),
        detail: detail.into(),
        weight: w,
    });
}

pub fn resolve(ctx: &ResolutionContext, sigs: &[AgentSignatureV2]) -> ResolverDecision {
    let mut matches: Vec<AgentMatch> = sigs
        .iter()
        .filter_map(|s| score_signature(ctx, s))
        .collect();
    matches.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let best = matches.first().cloned();
    let runner_up = matches.get(1).cloned();

    const CONFIRM_THRESHOLD: f64 = 0.75; // ต่ำกว่านี้ → ให้คนยืนยัน
    const AMBIGUITY_GAP: f64 = 0.15; // best กับที่สองใกล้กันเกินไป → กำกวม

    let needs_human = match (&best, &runner_up) {
        (Some(b), _) if b.confidence < CONFIRM_THRESHOLD => true,
        (Some(b), Some(r)) if (b.confidence - r.confidence) < AMBIGUITY_GAP => true,
        (None, _) => true, // ไม่ match อะไรเลย = Unknown → ให้คนช่วย
        _ => false,
    };
    let reason = describe(&best, &runner_up, needs_human);
    ResolverDecision {
        best,
        runner_up,
        needs_human,
        reason,
    }
}

fn describe(
    best: &Option<AgentMatch>,
    runner_up: &Option<AgentMatch>,
    needs_human: bool,
) -> String {
    if let Some(b) = best {
        if needs_human {
            if let Some(r) = runner_up {
                format!(
                    "Ambiguous match between {} ({:.2}) and {} ({:.2})",
                    b.display_name, b.confidence, r.display_name, r.confidence
                )
            } else {
                format!(
                    "Low confidence match for {} ({:.2})",
                    b.display_name, b.confidence
                )
            }
        } else {
            format!("Strong match for {} ({:.2})", b.display_name, b.confidence)
        }
    } else {
        "No matching signature found".into()
    }
}

/// fail-safe: ถ้าไม่ match signature เป๊ะ แต่มีร่องรอยตระกูล Claw → flag + ส่งเข้า HITL.
pub fn claw_family_heuristic(ctx: &ResolutionContext) -> Option<AgentMatch> {
    let hay = format!(
        "{} {} {}",
        ctx.cmd_redacted,
        ctx.exe_path_norm.clone().unwrap_or_default(),
        ctx.present_paths.join(" ")
    )
    .to_ascii_lowercase();

    // ตระกูล: *claw* / *paw* (openclaw, hiclaw, thclaw, qwenpaw, clawhub, ...)
    let hit = ["claw", "paw"].iter().any(|kw| {
        hay.contains(kw)
            || ctx
                .cli_on_path
                .iter()
                .any(|c| c.to_ascii_lowercase().contains(*kw))
    });
    if !hit {
        return None;
    }

    // ดึงชื่อ variant ออกมาเป็น guess (เช่น "thclaw")
    let guess = extract_claw_variant(&hay).unwrap_or_else(|| "unknown-claw".into());
    let pretty = match guess.as_str() {
        "openclaw" => "OpenClaw",
        "antigravity" => "Antigravity",
        "hiclaw" => "HiClaw",
        "qwenpaw" => "QwenPaw",
        "clawhub" => "ClawHub",
        other => other,
    };
    Some(AgentMatch {
        signature_id: format!("claw_family_generic:{guess}"),
        display_name: format!("{pretty} (Claw-family, unconfirmed)"),
        vendor: Some("claw-family".into()),
        product: Some(guess),
        agent_type: "automation_agent".into(),
        confidence: 0.55, // ต่ำ → บังคับเข้า HITL (needs_human)
        matched_signals: vec![MatchedSignal {
            kind: "claw_family_heuristic".into(),
            detail: "matched *claw*/*paw* pattern".into(),
            weight: 0.55,
        }],
        capability_tags: vec![
            "net.egress.llm".into(),
            "channel.messaging".into(),
            "fs.write".into(),
        ],
    })
}

fn extract_claw_variant(hay: &str) -> Option<String> {
    // แตกทั้ง whitespace และ path separator → ได้ segment จริง
    let segments: Vec<&str> = hay
        .split(|c: char| c.is_whitespace() || c == '/' || c == '\\')
        .collect();
    // 1) เจอ node_modules/<pkg> → เอา pkg
    for w in segments.windows(2) {
        if w[0] == "node_modules" && (w[1].contains("claw") || w[1].contains("paw")) {
            return Some(clean_token(w[1]));
        }
    }
    // 2) segment ใดที่ลงท้าย/มี claw|paw และสั้นพอเป็นชื่อ package
    for seg in &segments {
        let s = clean_token(seg);
        if (s.contains("claw") || s.contains("paw")) && s.len() <= 20 && !s.is_empty() {
            return Some(s);
        }
    }
    None
}

fn clean_token(t: &str) -> String {
    t.trim_end_matches(".js")
        .trim_end_matches(".exe")
        .replace(|c: char| !c.is_alphanumeric() && c != '-' && c != '_', "")
}

// Helpers for string matching
fn expand_path(p: &str) -> String {
    if let Some(stripped) = p.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return format!("{}/{}", home.display(), stripped);
        }
    }
    p.to_string()
}

fn regex_match(pat: &str, haystack: &str) -> bool {
    if let Ok(re) = regex::Regex::new(pat) {
        re.is_match(haystack)
    } else {
        false
    }
}

fn glob_match(pat: &str, haystack: &str) -> bool {
    if let Ok(pattern) = glob::Pattern::new(pat) {
        pattern.matches(haystack)
    } else {
        false
    }
}

pub fn resolve_installed_app(
    path: &str,
    sigs: &[dek_fingerprint_defs::model::InstalledAppSignatureDef],
) -> Option<AgentMatch> {
    for sig in sigs {
        for marker in &sig.markers {
            for mp in &marker.paths {
                let mut expanded = expand_path(mp);
                if expanded.contains("%LOCALAPPDATA%") {
                    if let Ok(val) = std::env::var("LOCALAPPDATA") {
                        expanded = expanded.replace("%LOCALAPPDATA%", &val);
                    }
                }
                if expanded.contains("%APPDATA%") {
                    if let Ok(val) = std::env::var("APPDATA") {
                        expanded = expanded.replace("%APPDATA%", &val);
                    }
                }

                let norm_path = path.replace("\\", "/").to_lowercase();
                let norm_expanded = expanded.replace("\\", "/").to_lowercase();

                if norm_path.starts_with(&norm_expanded) {
                    return Some(AgentMatch {
                        signature_id: sig.id.clone(),
                        display_name: sig.name.clone(),
                        vendor: Some(sig.vendor.clone()),
                        product: Some(sig.product.clone()),
                        agent_type: sig.agent_type.clone(),
                        confidence: 1.0,
                        matched_signals: vec![MatchedSignal {
                            kind: "installed_app".into(),
                            detail: format!("matched path: {}", mp),
                            weight: 1.0,
                        }],
                        capability_tags: sig.capability_tags.clone(),
                    });
                }
            }
        }
    }
    None
}
