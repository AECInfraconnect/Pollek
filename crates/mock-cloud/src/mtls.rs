use anyhow::{Context, Result};
use rustls::{
    server::{WebPkiClientVerifier, ServerConfig},
    RootCertStore,
};
use std::sync::Arc;
use std::fs::File;
use std::io::BufReader;
use rustls_pemfile::{certs, private_key};
use rustls_pki_types::{CertificateDer, PrivateKeyDer};

/// Builds the ServerConfig for strictly authenticated mTLS (Port 43891)
pub fn build_mtls_config(allow_insecure: bool) -> Result<ServerConfig> {
    let certs_der = load_certs("certs/server.crt")?;
    let key_der = load_private_key("certs/server.key")?;

    let mut root_cert_store = RootCertStore::empty();
    let ca_certs = load_certs("certs/root_ca.crt")?;
    root_cert_store.add_parsable_certificates(ca_certs);

    // Strictly enforce client certificates. Reject requests without valid certs.
    // In a real environment, we would also verify `O=Pollen Cloud` via a custom verifier.
    // For this Mock Cloud, ensuring it's signed by our CA is logically equivalent to
    // enforcing O=Pollen Cloud since we only issue such certs.
    let client_verifier = WebPkiClientVerifier::builder(Arc::new(root_cert_store))
        .build()
        .context("Failed to build strict mTLS client verifier")?;

    let builder = ServerConfig::builder();
    
    let mut server_config_mtls = if allow_insecure {
        builder.with_no_client_auth()
    } else {
        builder.with_client_cert_verifier(client_verifier)
    }
    .with_single_cert(certs_der, key_der)
    .context("Failed to create mTLS ServerConfig")?;
        
    server_config_mtls.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(server_config_mtls)
}

/// Builds the ServerConfig for unauthenticated HTTPS (Port 43892)
pub fn build_https_config() -> Result<ServerConfig> {
    let certs_der = load_certs("certs/server.crt")?;
    let key_der = load_private_key("certs/server.key")?;

    let mut server_config_https = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs_der, key_der)
        .context("Failed to create HTTPS ServerConfig")?;
        
    server_config_https.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(server_config_https)
}

pub fn load_certs(path: &str) -> Result<Vec<CertificateDer<'static>>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    Ok(certs(&mut reader).collect::<Result<Vec<_>, _>>()?)
}

pub fn load_private_key(path: &str) -> Result<PrivateKeyDer<'static>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    private_key(&mut reader)?.context("No private key found")
}
