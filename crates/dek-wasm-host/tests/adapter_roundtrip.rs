#![allow(clippy::unwrap_used, clippy::expect_used)]

use dek_wasm_host::{
    config::WasmHostConfig,
    host::WasmPluginHost,
    plugin_key::{sha256_hex, PluginKey},
};

#[tokio::test]
async fn migrates_bundle_via_real_wasm_adapter() {
    let wasm = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../local-control-plane/assets/contract-adapter.wasm"
    ))
    .expect("committed adapter wasm");
    let host = WasmPluginHost::new(WasmHostConfig::default()).unwrap();
    let sha = sha256_hex(&wasm);
    let key = PluginKey {
        tenant_id: "local".into(),
        plugin_id: "contract-adapter".into(),
        version: "0.1.0".into(),
        wasm_sha256: sha.clone(),
        abi_version: "1".into(),
    };
    host.load_plugin(key, &wasm).await.expect("load");
    let pool_key = format!("local:contract-adapter:0.1.0:{sha}");

    // A sparse bundle missing required contract fields.
    let input = serde_json::to_vec(&serde_json::json!({
        "to_contract": "2026.06.29",
        "bundle": { "metadata": { "bundle_id": "b1" } }
    }))
    .unwrap();
    let out = host
        .invoke(&pool_key, "req1".into(), &input, 100_000_000)
        .await
        .expect("invoke");
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["adapted"], true);
    assert_eq!(v["bundle"]["apiVersion"], "v1");
    assert_eq!(
        v["bundle"]["compatibility"]["contract_version"],
        "2026.06.29"
    );
    assert!(v["bundle"]["compatibility"]["required_os_modules"]["linux"].is_array());
    assert_eq!(v["bundle"]["activation"]["strategy"], "shadow");
    assert!(!v["changes"].as_array().unwrap().is_empty());
}
