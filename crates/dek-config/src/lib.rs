pub mod paths;
pub mod logging;

use anyhow::{Context, Result};
use reqwest::{Certificate, Identity};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MtlsConfig {
    pub client_cert_path: String,
    pub client_key_path: String,
    pub root_ca_path: String,
}

impl MtlsConfig {
    pub fn build_client(&self, client_key_override: Option<&[u8]>) -> Result<reqwest::Client> {
        let root_ca_der = fs::read(&self.root_ca_path).context(format!("Failed to read root CA from {}", self.root_ca_path))?;
        let root_ca = Certificate::from_pem(&root_ca_der)?;

        let client_cert = fs::read(&self.client_cert_path).context("Failed to read client cert")?;
        
        let client_key = match client_key_override {
            Some(key) => key.to_vec(),
            None => fs::read(&self.client_key_path).context("Failed to read client key")?
        };
        
        let mut id_pem = client_cert;
        id_pem.extend_from_slice(b"\n");
        id_pem.extend_from_slice(&client_key);
        let identity = Identity::from_pem(&id_pem)?;

        let client = reqwest::Client::builder()
            .add_root_certificate(root_ca)
            .identity(identity)
            .timeout(std::time::Duration::from_secs(10))
            .build()?;
        Ok(client)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapConfig {
    pub device_id: String,
    pub mtls: MtlsConfig,
    pub pinned_bundle_public_key: String,
}

impl BootstrapConfig {
    pub fn load_or_default(path: &str) -> Result<Self> {
        let p = Path::new(path);
        if p.exists() {
            let data = fs::read_to_string(p)?;
            let config: BootstrapConfig = serde_json::from_str(&data)?;
            Ok(config)
        } else {
            let default_config = Self {
                device_id: "device-001".to_string(),
                mtls: MtlsConfig {
                    client_cert_path: paths::get_config_dir().join("certs").join("client.crt").to_string_lossy().into_owned(),
                    client_key_path: paths::get_config_dir().join("certs").join("client.key").to_string_lossy().into_owned(),
                    root_ca_path: paths::get_config_dir().join("certs").join("root_ca.crt").to_string_lossy().into_owned(),
                },
                pinned_bundle_public_key: "xQyzrpVpR6jeGRNbW+JoX/NIr8Y/w0qDesoSvFwfViU="
                    .to_string(),
            };
            let json_str = serde_json::to_string_pretty(&default_config)?;
            if let Some(parent) = p.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if fs::write(p, json_str).is_ok() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = fs::set_permissions(p, fs::Permissions::from_mode(0o600));
                }
            }
            Ok(default_config)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenFgaConfig {
    pub endpoint: String,
    pub store_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CedarConfig {
    pub policy_src: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmConfig {
    pub policy_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfig {
    pub openfga: Option<OpenFgaConfig>,
    pub cedar: Option<CedarConfig>,
    pub opa_wasm: Option<WasmConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpireServerConfig {
    pub endpoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtConfig {
    pub public_key_pem: Option<String>,
    pub jwks: Option<serde_json::Value>,
    pub issuer_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfig {
    pub download_url: String,
    pub signature_b64: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DekConfig {
    pub device_id: String,
    pub tenant_id: String,
    pub mtls: MtlsConfig,
    pub spire_server: Option<SpireServerConfig>,
    pub policy_config: Option<PolicyConfig>,
    pub jwt_config: Option<JwtConfig>,
    pub update_config: Option<UpdateConfig>,
}

impl DekConfig {
    pub async fn fetch_from_cloud(
        bootstrap: &BootstrapConfig,
        endpoint_base: &str,
    ) -> Result<Self> {
        let client = bootstrap.mtls.build_client(None)?;

        let url = format!("{}/config/{}", endpoint_base, bootstrap.device_id);
        println!("Fetching dynamic config from cloud over MTLS: {}", url);

        let res = client.get(&url).send().await?;
        if !res.status().is_success() {
            anyhow::bail!("Failed to fetch config. Status: {}", res.status());
        }

        let config: DekConfig = res.json().await?;
        Ok(config)
    }
}
