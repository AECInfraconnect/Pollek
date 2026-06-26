use crate::model::*;
use anyhow::Result;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct SniFlow {
    pub browser_pid: Option<u32>,
    pub sni_host: String,
    pub ts: u64,
}

pub trait SniFlowSource: Send + Sync {
    /// คืน flow ที่ DEK เห็นในหน้าต่างเวลาล่าสุด
    fn recent_flows(&self, since: Duration) -> Vec<SniFlow>;
}

pub fn scan_web_ai(
    sni_source: Option<&dyn SniFlowSource>,
    config: &crate::config::DiscoveryConfig,
    catalog: &[dek_fingerprint_defs::model::WebAiSignatureDef],
) -> Result<Vec<DiscoveryEvidenceV2>> {
    let mut evidence = Vec::new();

    let consent_store_path = dek_config::paths::get_config_dir().join("consent.json");
    let consent_store = dek_consent::ConsentStore::new(consent_store_path);
    let has_consent = consent_store
        .has_consented(&dek_consent::AgreementType::BrowserHistoryScan)
        .unwrap_or(false);

    if config.enable_browser_history_scan && has_consent {
        if let Ok(mut hist) = scan_history(catalog) {
            evidence.append(&mut hist);
        }
        if let Ok(mut bm) = scan_bookmarks(catalog) {
            evidence.append(&mut bm);
        }
    }

    if config.enable_browser_session_scan {
        if let Ok(mut sess) = scan_sessions(catalog) {
            evidence.append(&mut sess);
        }
    }

    if config.enable_network_sni_scan {
        if let Some(source) = sni_source {
            if let Ok(mut net) = scan_network_sni(source, catalog) {
                evidence.append(&mut net);
            }
        }
    }

    Ok(evidence)
}

fn scan_history(
    catalog: &[dek_fingerprint_defs::model::WebAiSignatureDef],
) -> Result<Vec<DiscoveryEvidenceV2>> {
    let mut evidence = Vec::new();
    let mut seen = HashSet::new();
    let history_paths = get_browser_history_paths();

    for path in history_paths {
        if !path.exists() {
            continue;
        }
        let (browser_id, browser_name) = browser_from_path(&path);

        let temp_path = path.with_extension(format!("temp_{}", uuid::Uuid::new_v4()));
        // Copy to avoid SQLite lock
        if std::fs::copy(&path, &temp_path).is_err() {
            continue;
        }

        if let Ok(conn) = rusqlite::Connection::open_with_flags(
            &temp_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        ) {
            let stmt = conn.prepare("SELECT url, title, last_visit_time, visit_count FROM urls ORDER BY last_visit_time DESC LIMIT 1000");
            if let Ok(mut stmt) = stmt {
                let url_iter = stmt.query_map([], |row| row.get::<_, String>(0));

                if let Ok(url_iter) = url_iter {
                    for url_result in url_iter.flatten() {
                        if let Ok(parsed_url) = url::Url::parse(&url_result) {
                            if let Some(host) = parsed_url.host_str() {
                                for sig in catalog {
                                    if let Some(matched_domain) = host_matches_signature(host, sig)
                                    {
                                        let origin = format!("{}://{}", parsed_url.scheme(), host);
                                        let merge_key = web_ai_merge_key(sig, browser_id);

                                        if !seen.insert(merge_key.clone()) {
                                            continue;
                                        }

                                        evidence.push(DiscoveryEvidenceV2 {
                                            evidence_id: uuid::Uuid::new_v4().to_string(),
                                            source: EvidenceSource::BrowserHistory,
                                            confidence: 0.6,
                                            observed_at: chrono::Utc::now().to_rfc3339(),
                                            privacy_class: PrivacyClass::InternalMetadata,
                                            redacted: true,
                                            data: serde_json::json!({
                                                "origin": origin,
                                                "name": web_ai_display_name(sig, browser_name),
                                                "base_name": sig.name.clone(),
                                                "vendor": sig.vendor.clone(),
                                                "browser_id": browser_id,
                                                "browser_name": browser_name,
                                                "matched_domain": matched_domain.clone(),
                                                "capability_tags": sig.capability_tags.clone(),
                                            }),
                                            merge_key: Some(merge_key),
                                            source_path_hash: Some(
                                                crate::redaction::sha256_string(
                                                    &path.to_string_lossy(),
                                                ),
                                            ),
                                            source_path_redacted: Some(
                                                crate::redaction::redact_path_for_ui(
                                                    &path.to_string_lossy(),
                                                ),
                                            ),
                                        });
                                        tracing::debug!(%matched_domain, "matched web AI history entry");
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let _ = std::fs::remove_file(temp_path);
    }

    Ok(evidence)
}

fn scan_sessions(
    catalog: &[dek_fingerprint_defs::model::WebAiSignatureDef],
) -> Result<Vec<DiscoveryEvidenceV2>> {
    let mut evidence = Vec::new();
    let mut seen = HashSet::new();
    let mut session_paths = get_browser_session_paths();
    session_paths.extend(crate::browser_session_reader::firefox_session_paths());
    session_paths.sort();
    session_paths.dedup();

    for path in session_paths {
        if !path.exists() {
            continue;
        }

        // Since session paths can be directories (like `Sessions` folder in newer Chrome), we should read all files in it if it's a dir.
        let mut files_to_scan = Vec::new();
        if path.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&path) {
                for entry in entries.flatten() {
                    let file_type = entry.file_type();
                    let is_file = match file_type {
                        Ok(ft) => ft.is_file(),
                        Err(_) => {
                            if let Ok(meta) = std::fs::metadata(entry.path()) {
                                meta.file_type().is_file()
                            } else {
                                false
                            }
                        }
                    };

                    if is_file {
                        files_to_scan.push(entry.path());
                    }
                }
            }
        } else {
            files_to_scan.push(path);
        }

        for file_path in files_to_scan {
            let bytes = match read_session_file_bytes(&file_path) {
                Ok(b) => b,
                Err(e) => {
                    tracing::debug!(?file_path, %e, "skip session file");
                    continue;
                }
            };
            let (browser_id, browser_name) = browser_from_path(&file_path);
            for sig in catalog {
                if let Some(matched_domain) = bytes_match_signature(&bytes, sig) {
                    let merge_key = web_ai_merge_key(sig, browser_id);

                    if !seen.insert(merge_key.clone()) {
                        continue;
                    }

                    evidence.push(DiscoveryEvidenceV2 {
                        evidence_id: uuid::Uuid::new_v4().to_string(),
                        source: EvidenceSource::BrowserSession,
                        confidence: 0.85,
                        observed_at: chrono::Utc::now().to_rfc3339(),
                        privacy_class: PrivacyClass::InternalMetadata,
                        redacted: true,
                        data: serde_json::json!({
                        "origin": format!("https://{}", matched_domain),
                            "name": web_ai_display_name(sig, browser_name),
                            "base_name": sig.name.clone(),
                            "vendor": sig.vendor.clone(),
                            "browser_id": browser_id,
                            "browser_name": browser_name,
                            "capability_tags": sig.capability_tags.clone(),
                            "detected_via": "browser_session_open_tab",
                            "matched_domain": matched_domain,
                        }),
                        merge_key: Some(merge_key),
                        source_path_hash: Some(crate::redaction::sha256_string(
                            &file_path.to_string_lossy(),
                        )),
                        source_path_redacted: Some(crate::redaction::redact_path_for_ui(
                            &file_path.to_string_lossy(),
                        )),
                    });
                }
            }
        }
    }

    Ok(evidence)
}

fn scan_network_sni(
    source: &dyn SniFlowSource,
    catalog: &[dek_fingerprint_defs::model::WebAiSignatureDef],
) -> Result<Vec<DiscoveryEvidenceV2>> {
    let mut evidence = Vec::new();
    let mut seen = HashSet::new();

    // Query recent flows from the injected source (e.g., from spool or eBPF directly)
    let recent_snis = source.recent_flows(Duration::from_secs(3600));

    for flow in recent_snis {
        let browser_scope = flow
            .browser_pid
            .and_then(crate::browser_window_scan::browser_scope_for_pid)
            .map(|(browser_id, browser_name, process_name)| {
                (browser_id, browser_name, Some(process_name))
            })
            .or_else(|| {
                crate::browser_window_scan::single_running_browser_scope()
                    .map(|(browser_id, browser_name)| (browser_id, browser_name, None))
            });
        let (browser_id, browser_name, browser_process) =
            browser_scope.unwrap_or(("network", "Browser", None));

        for sig in catalog {
            if let Some(matched_domain) = host_matches_signature(&flow.sni_host, sig) {
                let merge_key = web_ai_merge_key(sig, browser_id);

                if !seen.insert(merge_key.clone()) {
                    continue;
                }

                evidence.push(DiscoveryEvidenceV2 {
                    evidence_id: uuid::Uuid::new_v4().to_string(),
                    source: EvidenceSource::NetworkSni,
                    confidence: 1.0,
                    observed_at: chrono::Utc::now().to_rfc3339(),
                    privacy_class: PrivacyClass::InternalMetadata,
                    redacted: true,
                    data: serde_json::json!({
                        "origin": format!("https://{}", matched_domain),
                        "sni": flow.sni_host.clone(),
                        "name": web_ai_display_name(sig, browser_name),
                        "base_name": sig.name.clone(),
                        "vendor": sig.vendor.clone(),
                        "browser_id": browser_id,
                        "browser_name": browser_name,
                        "browser": browser_process,
                        "capability_tags": sig.capability_tags.clone(),
                        "browser_pid": flow.browser_pid,
                        "matched_domain": matched_domain,
                    }),
                    merge_key: Some(merge_key),
                    source_path_hash: None,
                    source_path_redacted: Some("network:sni".to_string()),
                });
            }
        }
    }

    Ok(evidence)
}

fn scan_bookmarks(
    catalog: &[dek_fingerprint_defs::model::WebAiSignatureDef],
) -> Result<Vec<DiscoveryEvidenceV2>> {
    let mut evidence = Vec::new();
    let mut seen = HashSet::new();
    let bookmark_paths = get_browser_bookmark_paths();

    for path in bookmark_paths {
        if !path.exists() {
            continue;
        }
        let (browser_id, browser_name) = browser_from_path(&path);

        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) {
                // very rough text search to see if any domain is present
                // in real implementation, you'd traverse the bookmark tree
                let json_str = json.to_string();
                for sig in catalog {
                    if let Some(matched_domain) = bytes_match_signature(json_str.as_bytes(), sig) {
                        let merge_key = web_ai_merge_key(sig, browser_id);

                        if !seen.insert(merge_key.clone()) {
                            continue;
                        }

                        evidence.push(DiscoveryEvidenceV2 {
                            evidence_id: uuid::Uuid::new_v4().to_string(),
                            source: EvidenceSource::BrowserHistory, // close enough or new source
                            confidence: 0.5, // lower confidence for bookmarks
                            observed_at: chrono::Utc::now().to_rfc3339(),
                            privacy_class: PrivacyClass::InternalMetadata,
                            redacted: true,
                            data: serde_json::json!({
                                "origin": format!("https://{}", matched_domain),
                                "name": web_ai_display_name(sig, browser_name),
                                "base_name": sig.name.clone(),
                                "vendor": sig.vendor.clone(),
                                "browser_id": browser_id,
                                "browser_name": browser_name,
                                "capability_tags": sig.capability_tags.clone(),
                                "matched_domain": matched_domain,
                            }),
                            merge_key: Some(merge_key),
                            source_path_hash: Some(crate::redaction::sha256_string(
                                &path.to_string_lossy(),
                            )),
                            source_path_redacted: Some(crate::redaction::redact_path_for_ui(
                                &path.to_string_lossy(),
                            )),
                        });
                    }
                }
            }
        }
    }

    Ok(evidence)
}

fn web_ai_display_name(
    sig: &dek_fingerprint_defs::model::WebAiSignatureDef,
    browser_name: &str,
) -> String {
    crate::browser_window_scan::browser_scoped_ai_name(&sig.name, browser_name)
}

fn web_ai_merge_key(
    sig: &dek_fingerprint_defs::model::WebAiSignatureDef,
    browser_id: &str,
) -> String {
    format!("webai:{}:{browser_id}", sig.stable_id())
}

fn browser_from_path(path: &Path) -> (&'static str, &'static str) {
    let normalized = path
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();

    if normalized.contains("microsoft/edge")
        || normalized.contains("microsoft edge")
        || normalized.contains("microsoft-edge")
    {
        ("edge", "Edge")
    } else if normalized.contains("google/chrome")
        || normalized.contains("google chrome")
        || normalized.contains("google-chrome")
    {
        ("chrome", "Chrome")
    } else if normalized.contains("bravesoftware/brave-browser")
        || normalized.contains("brave-browser")
    {
        ("brave", "Brave")
    } else if normalized.contains("opera software")
        || normalized.contains("opera stable")
        || normalized.contains("opera gx")
    {
        ("opera", "Opera")
    } else if normalized.contains("vivaldi") {
        ("vivaldi", "Vivaldi")
    } else if normalized.contains("chromium") {
        ("chromium", "Chromium")
    } else if normalized.contains("mozilla/firefox")
        || normalized.contains(".mozilla/firefox")
        || normalized.contains("/firefox/")
    {
        ("firefox", "Firefox")
    } else if normalized.contains("safari") {
        ("safari", "Safari")
    } else {
        ("browser", "Browser")
    }
}

fn get_browser_bookmark_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    #[cfg(target_os = "windows")]
    if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
        paths.push(
            PathBuf::from(&localappdata)
                .join("Google")
                .join("Chrome")
                .join("User Data")
                .join("Default")
                .join("Bookmarks"),
        );
        paths.push(
            PathBuf::from(&localappdata)
                .join("Microsoft")
                .join("Edge")
                .join("User Data")
                .join("Default")
                .join("Bookmarks"),
        );
    }

    #[cfg(target_os = "macos")]
    if let Ok(home) = std::env::var("HOME") {
        paths.push(
            PathBuf::from(&home)
                .join("Library")
                .join("Application Support")
                .join("Google")
                .join("Chrome")
                .join("Default")
                .join("Bookmarks"),
        );
    }

    #[cfg(target_os = "linux")]
    if let Ok(home) = std::env::var("HOME") {
        paths.push(
            PathBuf::from(&home)
                .join(".config")
                .join("google-chrome")
                .join("Default")
                .join("Bookmarks"),
        );
    }

    paths
}

fn get_browser_history_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    #[cfg(target_os = "windows")]
    if let Ok(appdata) = std::env::var("APPDATA") {
        if let Ok(entries) = std::fs::read_dir(
            PathBuf::from(&appdata)
                .join("Mozilla")
                .join("Firefox")
                .join("Profiles"),
        ) {
            for entry in entries.flatten() {
                paths.push(entry.path().join("places.sqlite"));
            }
        }
    }

    #[cfg(target_os = "windows")]
    if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
        paths.push(
            PathBuf::from(&localappdata)
                .join("Google")
                .join("Chrome")
                .join("User Data")
                .join("Default")
                .join("History"),
        );
        paths.push(
            PathBuf::from(&localappdata)
                .join("Microsoft")
                .join("Edge")
                .join("User Data")
                .join("Default")
                .join("History"),
        );
    }

    #[cfg(target_os = "macos")]
    if let Ok(home) = std::env::var("HOME") {
        paths.push(
            PathBuf::from(&home)
                .join("Library")
                .join("Application Support")
                .join("Google")
                .join("Chrome")
                .join("Default")
                .join("History"),
        );
        if let Ok(entries) = std::fs::read_dir(
            PathBuf::from(&home)
                .join("Library")
                .join("Application Support")
                .join("Firefox")
                .join("Profiles"),
        ) {
            for entry in entries.flatten() {
                paths.push(entry.path().join("places.sqlite"));
            }
        }
    }

    #[cfg(target_os = "linux")]
    if let Ok(home) = std::env::var("HOME") {
        paths.push(
            PathBuf::from(&home)
                .join(".config")
                .join("google-chrome")
                .join("Default")
                .join("History"),
        );
        if let Ok(entries) =
            std::fs::read_dir(PathBuf::from(&home).join(".mozilla").join("firefox"))
        {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    paths.push(entry.path().join("places.sqlite"));
                }
            }
        }
    }

    paths
}

fn web_ai_domains(sig: &dek_fingerprint_defs::model::WebAiSignatureDef) -> Vec<&str> {
    let mut domains = sig.domains();

    match sig.domain.as_str() {
        "chatgpt.com" => domains.push("chat.openai.com"),
        "chat.openai.com" => domains.push("chatgpt.com"),
        "gemini.google.com" => {
            domains.push("bard.google.com");
            domains.push("aistudio.google.com");
        }
        "aistudio.google.com" => domains.push("gemini.google.com"),
        "chat.deepseek.com" => {
            domains.push("deepseek.com");
            domains.push("api.deepseek.com");
        }
        "deepseek.com" => domains.push("chat.deepseek.com"),
        _ => {}
    }

    domains.sort_unstable();
    domains.dedup();
    domains
}

fn host_matches_signature(
    host: &str,
    sig: &dek_fingerprint_defs::model::WebAiSignatureDef,
) -> Option<String> {
    let normalized_host = host.trim_end_matches('.').to_ascii_lowercase();

    web_ai_domains(sig).into_iter().find_map(|domain| {
        let normalized_domain = domain.to_ascii_lowercase();
        if normalized_host == normalized_domain
            || normalized_host.ends_with(&format!(".{normalized_domain}"))
        {
            Some(domain.to_string())
        } else {
            None
        }
    })
}

fn bytes_match_signature(
    bytes: &[u8],
    sig: &dek_fingerprint_defs::model::WebAiSignatureDef,
) -> Option<String> {
    web_ai_domains(sig)
        .into_iter()
        .find(|domain| crate::browser_session_reader::bytes_contain_domain(bytes, domain))
        .map(str::to_string)
}

fn read_session_file_bytes(path: &Path) -> Result<Vec<u8>> {
    match crate::browser_session_reader::read_session_bytes(path) {
        Ok(bytes) => Ok(bytes),
        Err(first_err) => {
            let file_name = path
                .file_name()
                .and_then(|v| v.to_str())
                .unwrap_or("session.bin");
            let temp_path = std::env::temp_dir().join(format!(
                "pollek_session_{}_{}",
                uuid::Uuid::new_v4(),
                file_name
            ));

            if std::fs::copy(path, &temp_path).is_err() {
                return Err(first_err);
            }

            let result = crate::browser_session_reader::read_session_bytes(&temp_path);
            let _ = std::fs::remove_file(temp_path);
            result
        }
    }
}

fn push_chromium_profile_session_paths(paths: &mut Vec<PathBuf>, profile_dir: PathBuf) {
    paths.push(profile_dir.join("Sessions"));
    paths.push(profile_dir.join("Current Session"));
    paths.push(profile_dir.join("Last Session"));
    paths.push(profile_dir.join("Current Tabs"));
    paths.push(profile_dir.join("Last Tabs"));
}

fn push_chromium_user_data_session_paths(paths: &mut Vec<PathBuf>, user_data_dir: PathBuf) {
    push_chromium_profile_session_paths(paths, user_data_dir.join("Default"));

    if let Ok(entries) = std::fs::read_dir(&user_data_dir) {
        for entry in entries.flatten() {
            let profile_dir = entry.path();
            if !profile_dir.is_dir() {
                continue;
            }

            let profile_name = profile_dir
                .file_name()
                .and_then(|v| v.to_str())
                .unwrap_or_default();
            let looks_like_profile =
                profile_name.starts_with("Profile ") || profile_dir.join("Sessions").exists();

            if looks_like_profile {
                push_chromium_profile_session_paths(paths, profile_dir);
            }
        }
    }
}

fn get_browser_session_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    #[cfg(target_os = "windows")]
    if let Ok(appdata) = std::env::var("APPDATA") {
        if let Ok(entries) = std::fs::read_dir(
            PathBuf::from(&appdata)
                .join("Mozilla")
                .join("Firefox")
                .join("Profiles"),
        ) {
            for entry in entries.flatten() {
                paths.push(entry.path().join("sessionstore.jsonlz4"));
                paths.push(
                    entry
                        .path()
                        .join("sessionstore-backups")
                        .join("recovery.jsonlz4"),
                );
            }
        }
    }

    #[cfg(target_os = "windows")]
    if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
        let localappdata = PathBuf::from(localappdata);
        push_chromium_user_data_session_paths(
            &mut paths,
            localappdata.join("Google").join("Chrome").join("User Data"),
        );
        push_chromium_user_data_session_paths(
            &mut paths,
            localappdata
                .join("Microsoft")
                .join("Edge")
                .join("User Data"),
        );
        push_chromium_user_data_session_paths(
            &mut paths,
            localappdata
                .join("BraveSoftware")
                .join("Brave-Browser")
                .join("User Data"),
        );
        push_chromium_user_data_session_paths(
            &mut paths,
            localappdata.join("Chromium").join("User Data"),
        );
        push_chromium_user_data_session_paths(
            &mut paths,
            localappdata.join("Vivaldi").join("User Data"),
        );
    }

    #[cfg(target_os = "windows")]
    if let Ok(appdata) = std::env::var("APPDATA") {
        let appdata = PathBuf::from(appdata);
        push_chromium_profile_session_paths(
            &mut paths,
            appdata.join("Opera Software").join("Opera Stable"),
        );
        push_chromium_profile_session_paths(
            &mut paths,
            appdata.join("Opera Software").join("Opera GX Stable"),
        );
    }

    #[cfg(target_os = "macos")]
    if let Ok(home) = std::env::var("HOME") {
        push_chromium_user_data_session_paths(
            &mut paths,
            PathBuf::from(&home)
                .join("Library")
                .join("Application Support")
                .join("Google")
                .join("Chrome"),
        );
        push_chromium_user_data_session_paths(
            &mut paths,
            PathBuf::from(&home)
                .join("Library")
                .join("Application Support")
                .join("Microsoft Edge"),
        );
        push_chromium_user_data_session_paths(
            &mut paths,
            PathBuf::from(&home)
                .join("Library")
                .join("Application Support")
                .join("BraveSoftware")
                .join("Brave-Browser"),
        );
        if let Ok(entries) = std::fs::read_dir(
            PathBuf::from(&home)
                .join("Library")
                .join("Application Support")
                .join("Firefox")
                .join("Profiles"),
        ) {
            for entry in entries.flatten() {
                paths.push(entry.path().join("sessionstore.jsonlz4"));
                paths.push(
                    entry
                        .path()
                        .join("sessionstore-backups")
                        .join("recovery.jsonlz4"),
                );
            }
        }
    }

    #[cfg(target_os = "linux")]
    if let Ok(home) = std::env::var("HOME") {
        push_chromium_user_data_session_paths(
            &mut paths,
            PathBuf::from(&home).join(".config").join("google-chrome"),
        );
        push_chromium_user_data_session_paths(
            &mut paths,
            PathBuf::from(&home).join(".config").join("chromium"),
        );
        push_chromium_user_data_session_paths(
            &mut paths,
            PathBuf::from(&home).join(".config").join("microsoft-edge"),
        );
        push_chromium_user_data_session_paths(
            &mut paths,
            PathBuf::from(&home)
                .join(".config")
                .join("BraveSoftware")
                .join("Brave-Browser"),
        );
        if let Ok(entries) =
            std::fs::read_dir(PathBuf::from(&home).join(".mozilla").join("firefox"))
        {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    paths.push(entry.path().join("sessionstore.jsonlz4"));
                    paths.push(
                        entry
                            .path()
                            .join("sessionstore-backups")
                            .join("recovery.jsonlz4"),
                    );
                }
            }
        }
    }

    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sig(domain: &str) -> dek_fingerprint_defs::model::WebAiSignatureDef {
        dek_fingerprint_defs::model::WebAiSignatureDef {
            id: domain.replace('.', "_"),
            domain: domain.to_string(),
            alias_domains: Vec::new(),
            name: "Test AI".to_string(),
            vendor: "Test".to_string(),
            title_patterns: Vec::new(),
            app_cmdline_patterns: Vec::new(),
            capability_tags: vec!["llm.chat".to_string()],
            risk_weight: 0.5,
        }
    }

    #[test]
    fn host_matching_requires_domain_boundary() {
        let signature = sig("chatgpt.com");

        assert_eq!(
            host_matches_signature("chatgpt.com", &signature).as_deref(),
            Some("chatgpt.com")
        );
        assert_eq!(
            host_matches_signature("www.chatgpt.com", &signature).as_deref(),
            Some("chatgpt.com")
        );
        assert!(host_matches_signature("notchatgpt.com", &signature).is_none());
    }

    #[test]
    fn signature_matching_handles_known_web_ai_aliases() {
        let chatgpt = sig("chatgpt.com");
        let deepseek = sig("chat.deepseek.com");
        let gemini = sig("gemini.google.com");

        assert_eq!(
            bytes_match_signature(b"https://chat.openai.com/c/123", &chatgpt).as_deref(),
            Some("chat.openai.com")
        );
        assert_eq!(
            host_matches_signature("deepseek.com", &deepseek).as_deref(),
            Some("deepseek.com")
        );
        assert_eq!(
            host_matches_signature("bard.google.com", &gemini).as_deref(),
            Some("bard.google.com")
        );
    }

    #[test]
    fn browser_paths_scope_web_ai_display_name() {
        let (browser_id, browser_name) = browser_from_path(Path::new(
            r"C:\Users\me\AppData\Local\Microsoft\Edge\User Data\Default\Sessions",
        ));
        let mut signature = sig("chatgpt.com");
        signature.name = "ChatGPT (Web)".to_string();

        assert_eq!(browser_id, "edge");
        assert_eq!(browser_name, "Edge");
        assert_eq!(
            web_ai_display_name(&signature, browser_name),
            "ChatGPT (Edge)"
        );
        assert_eq!(
            web_ai_merge_key(&signature, browser_id),
            "webai:chatgpt_com:edge"
        );
    }
}
