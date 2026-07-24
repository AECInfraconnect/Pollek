use std::net::SocketAddr;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct LocalControlPlaneConfig {
    pub bind_addr: SocketAddr,
    pub db_url: String,
    pub data_dir: PathBuf,
    pub dashboard_dir: PathBuf,
    pub auth_disabled: bool,
    pub cloud_url: Option<String>,
    pub cloud_api_key: Option<String>,
}

impl LocalControlPlaneConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let bind_addr = std::env::var("DEK_LCP_BIND")
            .unwrap_or_else(|_| "127.0.0.1:43891".to_string())
            .parse()?;

        let db_url = std::env::var("DEK_LCP_DB")
            .unwrap_or_else(|_| "sqlite://./pollek-local.db?mode=rwc".to_string());

        let data_dir = PathBuf::from(
            std::env::var("DEK_LCP_DATA").unwrap_or_else(|_| "./pollek-local-data".into()),
        );

        let dashboard_dir = resolve_dashboard_dir();

        let auth_disabled = std::env::var("DEK_LCP_AUTH_DISABLE").unwrap_or_default() == "1";

        let cloud_url = std::env::var("DEK_CLOUD_URL").ok();
        let cloud_api_key = std::env::var("DEK_CLOUD_API_KEY").ok();

        Ok(Self {
            bind_addr,
            db_url,
            data_dir,
            dashboard_dir,
            auth_disabled,
            cloud_url,
            cloud_api_key,
        })
    }
}

/// Directory name the packaged dashboard assets live under, next to the
/// platform's shared-data prefix (`<prefix>/share/pollek-dek/dashboard`,
/// `C:\Program Files\PollekDEK\dashboard`, …).
const DASHBOARD_SUBDIR: &str = "dashboard";

/// Resolve the directory that holds the built dashboard (`index.html` + assets).
///
/// Order of precedence:
/// 1. `DEK_DASHBOARD_DIR` — explicit operator override (used by dev scripts and
///    installers that place assets somewhere non-standard).
/// 2. A standard install location that actually contains `index.html`, probed
///    relative to the running executable first (so a relocatable install just
///    works) and then the per-OS system prefixes the packaging pipeline targets.
/// 3. The in-repo dev build output as a last resort (`cargo run` from a checkout).
fn resolve_dashboard_dir() -> PathBuf {
    let env_override = std::env::var("DEK_DASHBOARD_DIR")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf));
    select_dashboard_dir(
        env_override,
        &standard_dashboard_candidates(exe_dir.as_deref()),
        PathBuf::from("./apps/local-admin-dashboard/dist"),
    )
}

/// Build the ordered list of standard locations to probe for the dashboard,
/// given the directory the current executable lives in (when known).
fn standard_dashboard_candidates(exe_dir: Option<&Path>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(dir) = exe_dir {
        // Windows / portable layout: assets sit beside the binary
        // (`C:\Program Files\PollekDEK\dashboard`, `<unpacked>/dashboard`).
        candidates.push(dir.join(DASHBOARD_SUBDIR));
        // Unix FHS layout: `<prefix>/bin/local-control-plane` →
        // `<prefix>/share/pollek-dek/dashboard`.
        if let Some(prefix) = dir.parent() {
            candidates.push(prefix.join("share/pollek-dek").join(DASHBOARD_SUBDIR));
        }
    }

    #[cfg(target_os = "linux")]
    {
        candidates.push(PathBuf::from("/usr/share/pollek-dek").join(DASHBOARD_SUBDIR));
        candidates.push(PathBuf::from("/usr/local/share/pollek-dek").join(DASHBOARD_SUBDIR));
    }
    #[cfg(target_os = "macos")]
    {
        candidates.push(PathBuf::from("/usr/local/share/pollek-dek").join(DASHBOARD_SUBDIR));
        candidates
            .push(PathBuf::from("/Library/Application Support/PollekDEK").join(DASHBOARD_SUBDIR));
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(program_files) = std::env::var("ProgramFiles") {
            candidates.push(
                PathBuf::from(program_files)
                    .join("PollekDEK")
                    .join(DASHBOARD_SUBDIR),
            );
        }
    }

    candidates
}

/// Pure selection logic (no environment access) so it can be unit-tested:
/// an explicit override wins outright; otherwise the first candidate that
/// actually contains `index.html` is used; otherwise the dev fallback.
fn select_dashboard_dir(
    env_override: Option<String>,
    candidates: &[PathBuf],
    dev_fallback: PathBuf,
) -> PathBuf {
    if let Some(dir) = env_override {
        return PathBuf::from(dir);
    }
    for candidate in candidates {
        if candidate.join("index.html").is_file() {
            return candidate.clone();
        }
    }
    dev_fallback
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn env_override_wins_even_when_missing_on_disk() {
        let chosen = select_dashboard_dir(
            Some("/custom/dashboard".to_string()),
            &[PathBuf::from("/usr/share/pollek-dek/dashboard")],
            PathBuf::from("./dev"),
        );
        assert_eq!(chosen, PathBuf::from("/custom/dashboard"));
    }

    #[test]
    fn first_candidate_with_index_html_is_selected() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let empty = tmp.path().join("empty");
        let real = tmp.path().join("real");
        fs::create_dir_all(&empty)?;
        fs::create_dir_all(&real)?;
        fs::write(real.join("index.html"), b"<!doctype html>")?;

        let chosen =
            select_dashboard_dir(None, &[empty.clone(), real.clone()], PathBuf::from("./dev"));
        assert_eq!(chosen, real);
        Ok(())
    }

    #[test]
    fn falls_back_to_dev_dir_when_no_candidate_has_index() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let empty = tmp.path().join("empty");
        fs::create_dir_all(&empty)?;

        let chosen = select_dashboard_dir(
            None,
            &[empty],
            PathBuf::from("./apps/local-admin-dashboard/dist"),
        );
        assert_eq!(chosen, PathBuf::from("./apps/local-admin-dashboard/dist"));
        Ok(())
    }

    #[test]
    fn unix_candidates_include_relative_and_fhs_paths() {
        let candidates = standard_dashboard_candidates(Some(Path::new("/usr/bin")));
        assert!(candidates.contains(&PathBuf::from("/usr/bin/dashboard")));
        assert!(candidates.contains(&PathBuf::from("/usr/share/pollek-dek/dashboard")));
    }
}
