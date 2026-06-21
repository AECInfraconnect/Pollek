use std::net::SocketAddr;
use std::path::PathBuf;

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
            .unwrap_or_else(|_| "sqlite://./pollen-local.db?mode=rwc".to_string());

        let data_dir = PathBuf::from(
            std::env::var("DEK_LCP_DATA").unwrap_or_else(|_| "./pollen-local-data".into()),
        );

        let dashboard_dir = PathBuf::from(
            std::env::var("DEK_DASHBOARD_DIR")
                .unwrap_or_else(|_| "./apps/local-admin-dashboard/dist".into()),
        );

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
