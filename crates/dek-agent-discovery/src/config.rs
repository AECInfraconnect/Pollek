#[derive(Clone, Debug)]
pub struct DiscoveryConfig {
    pub min_fingerprint_confidence: f64,
    pub cost_alert_threshold_usd: f64,
    pub default_retention_days: u32,
    pub enable_browser_history_scan: bool,
    pub enable_browser_session_scan: bool,
    pub enable_network_sni_scan: bool,
    pub source_timeout_secs: u64,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            min_fingerprint_confidence: 0.5,
            cost_alert_threshold_usd: 25.0,
            default_retention_days: 14,
            enable_browser_history_scan: false, // Privacy guard: default to false requiring user consent
            enable_browser_session_scan: true, // Privacy guard: session scan is safe as it only sees open tabs
            enable_network_sni_scan: true,     // SNI does not expose private URLs, on by default
            source_timeout_secs: 5,
        }
    }
}
