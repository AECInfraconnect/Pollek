//! dek-enroll — first-run device enrollment for Pollen DEK (RFC 8628).
//!
//! Production hardening: the device flow runs over an unreliable network for up
//! to `expires_in` seconds. A single dropped packet must NOT abort enrollment.
//!  - One-shot calls (device_authorization, enroll) retry transient failures
//!    with exponential backoff + jitter, bounded by a RetryPolicy.
//!  - The token poll loop tolerates transient send/parse errors: it logs and
//!    keeps polling until the device-code deadline, only giving up on a terminal
//!    OAuth error (access_denied / expired_token) or the deadline.
//!
//! Side-effect-free: returns an [`Enrollment`]; the caller drives SPIRE + writes
//! bootstrap.

#![allow(clippy::unwrap_used, clippy::expect_used)]
use serde::Deserialize;
use std::future::Future;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tracing::{info, warn};

#[derive(Debug, Error)]
pub enum EnrollError {
    #[error("network error talking to {0}: {1}")]
    Network(String, String),
    #[error("device authorization rejected: {0}")]
    DeviceAuth(String),
    #[error("the user did not approve before the code expired")]
    Expired,
    #[error("authorization was denied by the user")]
    AccessDenied,
    #[error("enrollment endpoint failed: HTTP {0}")]
    EnrollHttp(u16),
    #[error("malformed response from {0}")]
    BadResponse(String),
    #[error("gave up after {0} attempts: {1}")]
    RetriesExhausted(u32, String),
}

impl EnrollError {
    /// Transient (worth retrying) vs terminal.
    fn retryable(&self) -> bool {
        matches!(
            self,
            EnrollError::Network(..) | EnrollError::BadResponse(..)
        )
    }

    pub fn into_envelope(self) -> dek_errors::ErrorEnvelope {
        use dek_errors::{ErrorDomain, ErrorEnvelope, RetryClass, SafetyAction};
        let (code, msg, retry, rem) = match &self {
            EnrollError::Network(u, e) => (
                "NETWORK_ERROR",
                format!("Network error talking to {}: {}", u, e),
                RetryClass::RetryWithBackoff,
                Some("Check your internet connection and proxy settings.".to_string())
            ),
            EnrollError::DeviceAuth(e) => (
                "DEVICE_AUTH_REJECTED",
                format!("Device authorization rejected: {}", e),
                RetryClass::NoRetry,
                Some("Ensure you are using the correct Pollen Cloud URL and your client ID is valid.".to_string())
            ),
            EnrollError::Expired => (
                "DEVICE_CODE_EXPIRED",
                "The user did not approve before the code expired".to_string(),
                RetryClass::RetryImmediate,
                Some("Run the enrollment command again and approve promptly.".to_string())
            ),
            EnrollError::AccessDenied => (
                "ACCESS_DENIED",
                "Authorization was denied by the user".to_string(),
                RetryClass::NoRetry,
                Some("User must approve the prompt to enroll the device.".to_string())
            ),
            EnrollError::EnrollHttp(s) => (
                "ENROLL_ENDPOINT_FAILED",
                format!("Enrollment endpoint failed: HTTP {}", s),
                if *s >= 500 { RetryClass::RetryWithBackoff } else { RetryClass::NoRetry },
                Some("Contact the cloud administrator if this persists.".to_string())
            ),
            EnrollError::BadResponse(s) => (
                "MALFORMED_RESPONSE",
                format!("Malformed response from {}", s),
                RetryClass::RetryWithBackoff,
                Some("The cloud service returned invalid data. Try again later.".to_string())
            ),
            EnrollError::RetriesExhausted(n, last) => (
                "RETRIES_EXHAUSTED",
                format!("Gave up after {} attempts. Last error: {}", n, last),
                RetryClass::NoRetry,
                Some("Check your network stability and try again.".to_string())
            ),
        };

        ErrorEnvelope {
            error_id: uuid::Uuid::new_v4().to_string(),
            domain: ErrorDomain::Enrollment,
            code: code.to_string(),
            message: msg.clone(),
            safe_message: msg,
            retry_class: retry,
            safety_action: SafetyAction::DenyRequest,
            tenant_id: None,
            device_id: None,
            bundle_version: None,
            request_id: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            remediation: rem,
        }
    }
}

/// Backoff configuration for transient failures.
#[derive(Clone)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(8),
        }
    }
}

/// What the caller needs to complete enrollment.
#[derive(Debug, Clone)]
pub struct Enrollment {
    pub join_token: String,
    pub spire_endpoint: String,
    pub trust_bundle_pem: String,
    pub pinned_bundle_public_key: String,
    pub tenant_id: String,
    pub device_id: String,
    pub spiffe_id_hint: Option<String>,
    pub cloud_url: String,
}

pub struct EnrollClient {
    cloud_url: String,
    client_id: String,
    scope: String,
    http: reqwest::Client,
    retry: RetryPolicy,
}

impl EnrollClient {
    pub fn new(cloud_url: &str, client_id: &str, scope: &str, ca_pem: Option<&str>) -> Self {
        let mut builder = reqwest::Client::builder().timeout(Duration::from_secs(15));
        if let Some(pem) = ca_pem {
            if let Ok(cert) = reqwest::Certificate::from_pem(pem.as_bytes()) {
                builder = builder.add_root_certificate(cert);
            }
        }
        Self {
            cloud_url: cloud_url.trim_end_matches('/').to_string(),
            client_id: client_id.to_string(),
            scope: scope.to_string(),
            http: builder.build().expect("build http client"),
            retry: RetryPolicy::default(),
        }
    }

    pub fn with_retry_policy(mut self, retry: RetryPolicy) -> Self {
        self.retry = retry;
        self
    }

    pub async fn run<F: Fn(&UserPrompt)>(&self, display: F) -> Result<Enrollment, EnrollError> {
        let auth = self.request_device_code().await?;
        display(&UserPrompt {
            verification_uri: auth.verification_uri.clone(),
            verification_uri_complete: auth.verification_uri_complete.clone(),
            user_code: auth.user_code.clone(),
            expires_in: auth.expires_in,
        });
        let access_token = self.poll_for_token(&auth).await?;
        self.enroll(&access_token).await
    }

    async fn request_device_code(&self) -> Result<DeviceAuthResp, EnrollError> {
        let url = format!("{}/oauth/device_authorization", self.cloud_url);
        self.with_retry("device_authorization", || async {
            let resp = self
                .http
                .post(&url)
                .form(&[
                    ("client_id", self.client_id.as_str()),
                    ("scope", self.scope.as_str()),
                ])
                .send()
                .await
                .map_err(|e| classify(&url, e))?;
            if !resp.status().is_success() {
                // 5xx is transient; 4xx is terminal config error.
                return if resp.status().is_server_error() {
                    Err(EnrollError::Network(
                        url.clone(),
                        format!("HTTP {}", resp.status()),
                    ))
                } else {
                    Err(EnrollError::DeviceAuth(format!("HTTP {}", resp.status())))
                };
            }
            resp.json::<DeviceAuthResp>()
                .await
                .map_err(|_| EnrollError::BadResponse("device_authorization".into()))
        })
        .await
    }

    async fn poll_for_token(&self, auth: &DeviceAuthResp) -> Result<String, EnrollError> {
        let url = format!("{}/oauth/token", self.cloud_url);
        let deadline = Instant::now() + Duration::from_secs(auth.expires_in);
        let mut interval = Duration::from_secs(auth.interval.unwrap_or(5).max(1));
        // Tolerate a run of transient blips before giving up early.
        let mut consecutive_transient = 0u32;
        const MAX_CONSECUTIVE_TRANSIENT: u32 = 10;

        info!(
            "waiting for user authorization (poll every {}s)...",
            interval.as_secs()
        );
        loop {
            if Instant::now() >= deadline {
                return Err(EnrollError::Expired);
            }
            tokio::time::sleep(interval).await;

            let send = self
                .http
                .post(&url)
                .form(&[
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                    ("device_code", auth.device_code.as_str()),
                    ("client_id", self.client_id.as_str()),
                ])
                .send()
                .await;

            // Network blip on a single poll => log + keep polling (don't abort).
            let resp = match send {
                Ok(r) => r,
                Err(e) => {
                    consecutive_transient += 1;
                    warn!(
                        "token poll network error ({}/{}): {}",
                        consecutive_transient, MAX_CONSECUTIVE_TRANSIENT, e
                    );
                    if consecutive_transient >= MAX_CONSECUTIVE_TRANSIENT {
                        return Err(EnrollError::Network(url.clone(), e.to_string()));
                    }
                    continue;
                }
            };

            let body: TokenResp = match resp.json().await {
                Ok(b) => b,
                Err(_) => {
                    consecutive_transient += 1;
                    warn!(
                        "token poll bad response ({}/{})",
                        consecutive_transient, MAX_CONSECUTIVE_TRANSIENT
                    );
                    if consecutive_transient >= MAX_CONSECUTIVE_TRANSIENT {
                        return Err(EnrollError::BadResponse("token".into()));
                    }
                    continue;
                }
            };
            consecutive_transient = 0; // got a well-formed reply

            if let Some(token) = body.access_token {
                info!("authorization granted");
                return Ok(token);
            }
            match body.error.as_deref() {
                Some("authorization_pending") => {}
                Some("slow_down") => {
                    interval += Duration::from_secs(5); // RFC 8628 §3.5
                    warn!(
                        "server asked to slow down; interval now {}s",
                        interval.as_secs()
                    );
                }
                Some("access_denied") => return Err(EnrollError::AccessDenied),
                Some("expired_token") => return Err(EnrollError::Expired),
                Some(other) => return Err(EnrollError::DeviceAuth(other.to_string())),
                None => {
                    return Err(EnrollError::BadResponse(
                        "token (no token, no error)".into(),
                    ))
                }
            }
        }
    }

    async fn enroll(&self, access_token: &str) -> Result<Enrollment, EnrollError> {
        let url = format!("{}/enroll", self.cloud_url);
        let r = self
            .with_retry("enroll", || async {
                let os = std::env::consts::OS;
                let arch = std::env::consts::ARCH;
                let hostname = gethostname::gethostname().to_string_lossy().into_owned();

                let payload = serde_json::json!({
                    "os": os,
                    "arch": arch,
                    "hostname": hostname,
                    "capabilities": dek_domain_schema::EnforcementCapabilities::detect()
                });

                let resp = self
                    .http
                    .post(&url)
                    .bearer_auth(access_token)
                    .json(&payload)
                    .send()
                    .await
                    .map_err(|e| classify(&url, e))?;
                if !resp.status().is_success() {
                    return if resp.status().is_server_error() {
                        Err(EnrollError::Network(
                            url.clone(),
                            format!("HTTP {}", resp.status()),
                        ))
                    } else {
                        Err(EnrollError::EnrollHttp(resp.status().as_u16()))
                    };
                }
                resp.json::<EnrollResp>()
                    .await
                    .map_err(|_| EnrollError::BadResponse("enroll".into()))
            })
            .await?;
        Ok(Enrollment {
            join_token: r.join_token,
            spire_endpoint: r.spire_endpoint,
            trust_bundle_pem: r.trust_bundle_pem,
            pinned_bundle_public_key: r.pinned_bundle_public_key,
            tenant_id: r.tenant_id,
            device_id: r.device_id,
            spiffe_id_hint: r.spiffe_id,
            cloud_url: r.cloud_url.unwrap_or_else(|| self.cloud_url.clone()),
        })
    }

    /// Retry a one-shot op on transient errors with exponential backoff + jitter.
    async fn with_retry<T, F, Fut>(&self, what: &str, op: F) -> Result<T, EnrollError>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T, EnrollError>>,
    {
        let mut last = String::new();
        for attempt in 0..self.retry.max_attempts {
            match op().await {
                Ok(v) => return Ok(v),
                Err(e) if e.retryable() => {
                    last = e.to_string();
                    let delay = self.backoff(attempt);
                    warn!(
                        "{what} failed (attempt {}/{}): {e}; retrying in {:?}",
                        attempt + 1,
                        self.retry.max_attempts,
                        delay
                    );
                    tokio::time::sleep(delay).await;
                }
                Err(e) => return Err(e), // terminal
            }
        }
        Err(EnrollError::RetriesExhausted(self.retry.max_attempts, last))
    }

    fn backoff(&self, attempt: u32) -> Duration {
        let exp = self
            .retry
            .base_delay
            .saturating_mul(2u32.saturating_pow(attempt));
        let capped = exp.min(self.retry.max_delay);
        // full jitter in [0, capped]
        let upper = capped.as_millis() as u64;
        Duration::from_millis(jitter_ms(upper.max(1)))
    }
}

/// Map a reqwest transport error to EnrollError. timeouts/connect failures are
/// transient (Network => retryable); a non-network request error is also wrapped
/// as Network here since at the send stage there's no HTTP status to act on.
fn classify(url: &str, e: reqwest::Error) -> EnrollError {
    EnrollError::Network(url.to_string(), e.to_string())
}

/// Cheap full-jitter without a rand dependency.
fn jitter_ms(upper: u64) -> u64 {
    if upper == 0 {
        return 0;
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u64;
    nanos % upper
}

pub struct UserPrompt {
    pub verification_uri: String,
    pub verification_uri_complete: Option<String>,
    pub user_code: String,
    pub expires_in: u64,
}

#[derive(Debug, Deserialize)]
struct DeviceAuthResp {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[serde(default)]
    verification_uri_complete: Option<String>,
    expires_in: u64,
    #[serde(default)]
    interval: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct TokenResp {
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EnrollResp {
    join_token: String,
    spire_endpoint: String,
    trust_bundle_pem: String,
    pinned_bundle_public_key: String,
    tenant_id: String,
    device_id: String,
    #[serde(default)]
    spiffe_id: Option<String>,
    #[serde(default)]
    cloud_url: Option<String>,
}
