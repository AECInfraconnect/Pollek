// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

//! svid_renewal.rs — keep the device's X.509-SVID fresh.
//!
//! SPIRE issues short-lived SVIDs. The join token from enrollment is one-time,
//! so renewal authenticates with the CURRENT SVID (mTLS) and asks the server to
//! sign a fresh CSR. After renewal we atomically rewrite the cert/key and
//! hot-swap every mTLS client (telemetry sink, bundle agent, metrics client)
//! via the existing `update_mtls` machinery — no restart needed.
//!
//! Fail-open: any renewal failure keeps the current SVID and retries with
//! backoff; it never crashes the daemon.

use anyhow::{Context, Result};
use dek_bundle_sync::BundleSyncAgent;
use dek_config::MtlsConfig;
use dek_telemetry::CloudTelemetrySink;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

#[derive(Clone)]
pub struct RenewalConfig {
    /// `{cloud_url}/spire/svid/renew`
    pub renew_url: String,
    pub device_id: String,
    /// Current identity paths (cert/key are rewritten in place on renewal).
    pub mtls: MtlsConfig,
}

const MIN_SLEEP: Duration = Duration::from_secs(60);
const RETRY_BACKOFF: Duration = Duration::from_secs(300);
/// Used when the cert can't be parsed for an expiry.
const FALLBACK_SLEEP: Duration = Duration::from_secs(1800);

pub fn spawn_svid_renewal_task(
    cancel: CancellationToken,
    cfg: RenewalConfig,
    telemetry_sink: Arc<CloudTelemetrySink>,
    bundle_agent: Arc<BundleSyncAgent>,
    metrics_client: Arc<RwLock<reqwest::Client>>,
    health_tx: tokio::sync::watch::Sender<crate::svid_renewal_failclosed::IdentityHealth>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        info!("SVID auto-renewal task started");
        loop {
            // 1) Schedule next renewal and update health state.
            let mut remaining_secs = 0;
            let sleep_dur = match seconds_until_renewal(&cfg.mtls.client_cert_path) {
                Ok((d, expires)) => {
                    remaining_secs = expires - now_secs();
                    let health = crate::svid_renewal_failclosed::classify(remaining_secs, true);
                    let _ = health_tx.send(health);
                    d
                }
                Err(e) => {
                    warn!(
                        "could not read SVID expiry ({e}); will retry in {:?}",
                        FALLBACK_SLEEP
                    );
                    let health = crate::svid_renewal_failclosed::classify(0, false);
                    let _ = health_tx.send(health);
                    FALLBACK_SLEEP
                }
            };
            info!("next SVID renewal in {}s", sleep_dur.as_secs());

            tokio::select! {
                _ = cancel.cancelled() => { info!("SVID renewal task shutting down."); break; }
                _ = tokio::time::sleep(sleep_dur) => {}
            }

            // 2) Renew. Fail-open with backoff.
            match renew_once(&cfg, &telemetry_sink, &bundle_agent, &metrics_client).await {
                Ok(spiffe) => {
                    metrics::counter!("dek_svid_renew_total").increment(1);
                    info!("SVID renewed successfully: {}", spiffe);
                }
                Err(e) => {
                    metrics::counter!("dek_svid_renew_errors_total").increment(1);
                    error!(
                        "SVID renewal failed: {e}; keeping current SVID, retry in {:?}",
                        RETRY_BACKOFF
                    );
                    // The time we actually slept
                    let slept = sleep_dur.as_secs() as i64;
                    let new_remaining = (remaining_secs - slept).max(0);
                    let health = crate::svid_renewal_failclosed::classify(new_remaining, false);
                    let _ = health_tx.send(health);

                    tokio::select! {
                        _ = cancel.cancelled() => break,
                        _ = tokio::time::sleep(RETRY_BACKOFF) => {}
                    }
                }
            }
        }
    })
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Parse the leaf cert's `notAfter` and return (sleep_duration, not_after_unix).
/// Sleeps until 10 minutes before expiry.
fn seconds_until_renewal(cert_path: &str) -> Result<(Duration, i64)> {
    let pem = std::fs::read(cert_path).context("read SVID cert")?;
    let (_, p) =
        x509_parser::pem::parse_x509_pem(&pem).map_err(|e| anyhow::anyhow!("PEM parse: {e}"))?;
    let cert = p.parse_x509().context("parse X.509")?;
    let not_after = cert.validity().not_after.timestamp();
    let remaining = not_after - now_secs();
    metrics::gauge!("dek_svid_expiry_seconds").set(remaining as f64);
    if remaining <= 600 {
        return Ok((MIN_SLEEP, not_after)); // <= 10m remaining — renew promptly
    }
    let renew_in = (remaining - 600).max(MIN_SLEEP.as_secs() as i64);
    Ok((Duration::from_secs(renew_in as u64), not_after))
}

async fn renew_once(
    cfg: &RenewalConfig,
    sink: &Arc<CloudTelemetrySink>,
    bundle_agent: &Arc<BundleSyncAgent>,
    metrics_client: &Arc<RwLock<reqwest::Client>>,
) -> Result<String> {
    // Authenticate the renewal with the CURRENT SVID; fetch + install new cert/key.
    let client = cfg
        .mtls
        .build_client(None)
        .context("build current mTLS client")?;
    let spiffe_id = fetch_and_install_svid(
        &cfg.renew_url,
        &client,
        &cfg.device_id,
        &cfg.mtls.client_cert_path,
        &cfg.mtls.client_key_path,
    )
    .await?;

    // Hot-swap every mTLS client to the renewed identity.
    let ks = dek_keystore::get_keystore();
    if let Ok(key) = std::fs::read(&cfg.mtls.client_key_path) {
        let _ = ks.store_key("mtls_client_key", &key);
    }
    let _ = sink.update_mtls(&cfg.mtls).await;
    let _ = bundle_agent.update_mtls(&cfg.mtls).await;
    if let Ok(c) = cfg.mtls.build_client(None) {
        *metrics_client.write().await = c;
    }
    Ok(spiffe_id)
}

/// Force a renewal explicitly (e.g. via IPC RotateIdentity).
pub async fn force_renew(
    cfg: &RenewalConfig,
    sink: &Arc<CloudTelemetrySink>,
    bundle_agent: &Arc<BundleSyncAgent>,
    metrics_client: &Arc<RwLock<reqwest::Client>>,
) -> Result<String> {
    renew_once(cfg, sink, bundle_agent, metrics_client).await
}

/// Renew the SVID and atomically install the new key+cert at the given paths.
/// Returns the new SPIFFE ID. Pure I/O — no client hot-swap — so it is unit
/// testable in isolation.
pub async fn fetch_and_install_svid(
    renew_url: &str,
    mtls_client: &reqwest::Client,
    device_id: &str,
    cert_path: &str,
    key_path: &str,
) -> Result<String> {
    let svid = dek_spire_node::renew_svid(renew_url, mtls_client, device_id)
        .await
        .context("renew_svid")?;
    // Write key first, then cert, both atomically.
    atomic_write(key_path, svid.key_pem.as_bytes())?;
    atomic_write(cert_path, svid.cert_pem.as_bytes())?;
    Ok(svid.spiffe_id)
}

fn atomic_write(path: &str, data: &[u8]) -> Result<()> {
    let tmp = format!("{path}.tmp");
    std::fs::write(&tmp, data).with_context(|| format!("write {tmp}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    std::fs::rename(&tmp, path).with_context(|| format!("rename -> {path}"))?;
    Ok(())
}

// ============================================================================
// Tests — run with: cargo test -p dek-core svid_renewal
// dek-core/Cargo.toml [dev-dependencies]:
//   tokio = { workspace = true, features = ["full"] }
//   axum = "0.7"
//   rcgen = "0.11"
//   time = "0.3"
//   tempfile = "3"
//   serde = { workspace = true, features = ["derive"] }
//   serde_json = { workspace = true }
//   x509-parser = "0.16"
// ============================================================================
#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use std::sync::{Arc, Mutex};

    // ---- scheduler logic: half-life, floored at MIN_SLEEP ----
    #[test]
    fn renewal_schedules_at_half_life_and_floors() {
        let dir = tempfile::tempdir().unwrap();
        let (ca, ca_key) = make_ca();

        // long-lived (~1000s) -> sleep ~400s (1000 - 600)
        let long = sign_leaf(&ca, &ca_key, "device-x", time_offset(1000));
        let p_long = dir.path().join("long.crt");
        std::fs::write(&p_long, &long).unwrap();
        let (d, _) = seconds_until_renewal(p_long.to_str().unwrap()).unwrap();
        assert!(
            d.as_secs() >= 390 && d.as_secs() <= 410,
            "got {}s",
            d.as_secs()
        );

        // near-expired -> floored at MIN_SLEEP (60s)
        let short = sign_leaf(&ca, &ca_key, "device-x", time_offset(2));
        let p_short = dir.path().join("short.crt");
        std::fs::write(&p_short, &short).unwrap();
        let (d2, _) = seconds_until_renewal(p_short.to_str().unwrap()).unwrap();
        assert_eq!(d2, MIN_SLEEP);
    }

    // ---- renewal path: short-lived cert is swapped for a fresh one ----
    #[tokio::test]
    async fn renewal_swaps_cert_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let (ca, ca_key) = make_ca();

        // 1) initial identity on disk: a plain client cert (NO spiffe SAN), short-lived.
        let cert_path = dir.path().join("client.crt");
        let key_path = dir.path().join("client.key");
        let ca_path = dir.path().join("root_ca.crt");
        let (init_cert, init_key) = make_client(&ca, &ca_key, "device-renew-1");
        std::fs::write(&cert_path, &init_cert).unwrap();
        std::fs::write(&key_path, &init_key).unwrap();
        std::fs::write(&ca_path, ca.pem()).unwrap();
        let before = std::fs::read_to_string(&cert_path).unwrap();
        assert!(
            !before.contains("spiffe"),
            "precondition: initial cert has no spiffe SAN"
        );

        // 2) spawn mock renew endpoint that signs the CSR (short-lived, spiffe SAN).
        let ca_pem = ca.pem();
        let ca_key_pem = ca_key.serialize_pem();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let state = MockState {
            ca_pem: ca_pem.clone(),
            ca_key_pem,
        };
        let app = axum::Router::new()
            .route("/spire/svid/renew", axum::routing::post(renew_handler))
            .with_state(Arc::new(state));
        tokio::spawn(async move {
            axum::serve(listener, app.into_make_service())
                .await
                .unwrap();
        });

        // 3) build an mTLS client from the on-disk identity (unused over http, but
        //    proves the current-SVID path) and run the install.
        let mtls = dek_config::MtlsConfig {
            client_cert_path: cert_path.to_string_lossy().into_owned(),
            client_key_path: key_path.to_string_lossy().into_owned(),
            root_ca_path: ca_path.to_string_lossy().into_owned(),
        };
        let client = mtls
            .build_client(None)
            .expect("build mtls client from disk identity");
        let renew_url = format!("http://{addr}/spire/svid/renew");

        let spiffe = fetch_and_install_svid(
            &renew_url,
            &client,
            "device-renew-1",
            mtls.client_cert_path.as_str(),
            mtls.client_key_path.as_str(),
        )
        .await
        .expect("renewal should install a new SVID");

        // 4) assert the cert on disk was swapped for the new SVID.
        let after = std::fs::read_to_string(&cert_path).unwrap();
        assert_ne!(before, after, "cert file must change after renewal");
        assert!(spiffe.starts_with("spiffe://"));
        assert!(spiffe.contains("device-renew-1"));

        let (_, pem) = x509_parser::pem::parse_x509_pem(after.as_bytes()).unwrap();
        let cert = pem.parse_x509().unwrap();
        let has_spiffe = cert
            .subject_alternative_name()
            .ok()
            .flatten()
            .map(|san| {
                san.value.general_names.iter().any(|gn| {
                    matches!(gn, x509_parser::extensions::GeneralName::URI(u) if u.starts_with("spiffe://"))
                })
            })
            .unwrap_or(false);
        assert!(has_spiffe, "renewed cert must carry the spiffe:// URI SAN");
    }

    // ---------- mock renew endpoint ----------
    struct MockState {
        ca_pem: String,
        ca_key_pem: String,
    }
    #[derive(serde::Deserialize)]
    struct RenewReq {
        device_id: String,
        csr_pem: String,
    }
    async fn renew_handler(
        axum::extract::State(st): axum::extract::State<Arc<MockState>>,
        axum::Json(req): axum::Json<RenewReq>,
    ) -> axum::Json<serde_json::Value> {
        let spiffe = format!("spiffe://pollen.test/tenant/device/{}", req.device_id);
        let cert = sign_csr(&st.ca_pem, &st.ca_key_pem, &req.csr_pem, &spiffe, 2);
        axum::Json(serde_json::json!({
            "svid_cert_pem": cert, "spiffe_id": spiffe, "trust_bundle_pem": st.ca_pem
        }))
    }

    // ---------- rcgen 0.13 test crypto ----------
    static CA_LOCK: Mutex<()> = Mutex::new(());

    fn time_offset(secs: i64) -> time::OffsetDateTime {
        time::OffsetDateTime::now_utc() + time::Duration::seconds(secs)
    }

    fn make_ca() -> (rcgen::Certificate, rcgen::KeyPair) {
        let _g = CA_LOCK.lock().unwrap();
        use rcgen::{BasicConstraints, CertificateParams, IsCa, KeyUsagePurpose};
        let mut p = CertificateParams::new(vec!["Pollen Test Root CA".to_string()]).unwrap();
        p.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        p.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
        let key_pair = rcgen::KeyPair::generate().unwrap();
        let cert = p.self_signed(&key_pair).unwrap();
        (cert, key_pair)
    }

    fn make_client(ca: &rcgen::Certificate, ca_key: &rcgen::KeyPair, cn: &str) -> (String, String) {
        use rcgen::CertificateParams;
        let p = CertificateParams::new(vec![cn.to_string()]).unwrap();
        let key_pair = rcgen::KeyPair::generate().unwrap();
        let cert = p.signed_by(&key_pair, ca, ca_key).unwrap();
        (cert.pem(), key_pair.serialize_pem())
    }

    fn sign_leaf(
        ca: &rcgen::Certificate,
        ca_key: &rcgen::KeyPair,
        cn: &str,
        not_after: time::OffsetDateTime,
    ) -> String {
        use rcgen::CertificateParams;
        let mut p = CertificateParams::new(vec![cn.to_string()]).unwrap();
        p.not_after = not_after;
        let key_pair = rcgen::KeyPair::generate().unwrap();
        p.signed_by(&key_pair, ca, ca_key).unwrap().pem()
    }

    fn sign_csr(
        ca_pem: &str,
        ca_key_pem: &str,
        csr_pem: &str,
        spiffe: &str,
        ttl_secs: i64,
    ) -> String {
        use rcgen::{CertificateParams, CertificateSigningRequestParams, KeyPair, SanType};
        let ca_key = KeyPair::from_pem(ca_key_pem).unwrap();
        let ca_params = CertificateParams::from_ca_cert_pem(ca_pem).unwrap();
        let ca = ca_params.self_signed(&ca_key).unwrap();
        let mut csr = CertificateSigningRequestParams::from_pem(csr_pem).unwrap();
        csr.params
            .subject_alt_names
            .push(SanType::URI(spiffe.try_into().unwrap()));
        csr.params.not_after = time_offset(ttl_secs);
        csr.params
            .signed_by(&csr.public_key, &ca, &ca_key)
            .unwrap()
            .pem()
    }
}
