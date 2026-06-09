use dek_cedar::CedarAdapter;
use dek_config::MtlsConfig;
use dek_openfga::OpenFgaAdapter;
use dek_policy_router::PolicyRouter;
use serde_json::Value;
use tracing::error;

pub fn load_router_config(router: &mut PolicyRouter, payload: &Value) {
    let mtls: Option<MtlsConfig> = payload
        .get("mtls")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    if let Some(scale_val) = payload.get("scale") {
        if let Ok(scale) = serde_json::from_value::<dek_config::ScaleConfig>(scale_val.clone()) {
            router.set_scale_config(
                scale.pdp_timeout_ms,
                scale.breaker_failure_threshold,
                scale.breaker_cooldown_secs,
            );
        }
    }

    if let Some(openfga) = payload.get("openfga") {
        let endpoint = openfga
            .get("endpoint")
            .and_then(|v| v.as_str())
            .unwrap_or("http://localhost:8080");
        let store_id = openfga
            .get("store_id")
            .and_then(|v| v.as_str())
            .unwrap_or("store_123");

        match OpenFgaAdapter::new(endpoint, store_id, mtls.as_ref()) {
            Ok(adapter) => router.register_evaluator("openfga", Box::new(adapter)),
            Err(e) => error!("Failed to initialize OpenFGA Adapter with mTLS: {}", e),
        }
    }
    if let Some(cedar) = payload.get("cedar") {
        let policy_src = cedar
            .get("policy_src")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        match CedarAdapter::new(policy_src) {
            Ok(adapter) => router.register_evaluator("cedar", Box::new(adapter)),
            Err(e) => error!("Failed to initialize Cedar Adapter: {}", e),
        }
    }
    if let Some(wasm) = payload.get("opa_wasm") {
        let policy_path = wasm
            .get("policy_path")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if std::path::Path::new(policy_path).exists() {
            if let Ok(runtime) = dek_policy_runtime::WasmtimePolicyRuntime::new(policy_path, None) {
                router.register_evaluator("opa_wasm", Box::new(runtime));
            } else {
                error!(
                    "Failed to initialize WASM Policy Runtime for path: {}",
                    policy_path
                );
            }
        } else {
            error!("WASM policy file not found at: {}", policy_path);
        }
    }

    if let Some(routes_val) = payload.get("routes") {
        match serde_json::from_value::<Vec<dek_policy_router::Route>>(routes_val.clone()) {
            Ok(routes) => {
                router.set_routes(routes);
            }
            Err(e) => {
                error!("Failed to parse routes from bundle: {} (routes_val: {})", e, routes_val);
            }
        }
    }
}
