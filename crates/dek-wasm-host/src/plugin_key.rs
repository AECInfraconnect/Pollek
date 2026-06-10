use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PluginKey {
    pub tenant_id: String,
    pub plugin_id: String,
    pub version: String,
    pub wasm_sha256: String,
    pub abi_version: String,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}
