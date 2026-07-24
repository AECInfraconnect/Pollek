use axum::{
    routing::{delete, get, post},
    Router,
};
use serde_json::Value;

use crate::state::AppState;

mod catalog;
mod lifecycle;

use catalog::{list_marketplace_items, marketplace_item_detail};
use lifecycle::{
    canary_plugin, disable_plugin, enable_plugin, health_plugin, install_plugin, list_plugins,
    revoke_plugin, rollback_plugin, test_plugin, toggle_plugin, uninstall_plugin, update_plugin,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/tenants/:tenant/marketplace/items",
            get(list_marketplace_items),
        )
        .route(
            "/v1/tenants/:tenant/marketplace/items/:id",
            get(marketplace_item_detail),
        )
        .route("/v1/tenants/:tenant/plugins", get(list_plugins))
        .route("/v1/tenants/:tenant/plugins/install", post(install_plugin))
        .route("/v1/tenants/:tenant/plugins/:id", delete(uninstall_plugin))
        .route(
            "/v1/tenants/:tenant/plugins/:id/toggle",
            post(toggle_plugin),
        )
        .route(
            "/v1/tenants/:tenant/plugins/:id/enable",
            post(enable_plugin),
        )
        .route(
            "/v1/tenants/:tenant/plugins/:id/disable",
            post(disable_plugin),
        )
        .route("/v1/tenants/:tenant/plugins/:id/test", post(test_plugin))
        .route(
            "/v1/tenants/:tenant/plugins/:id/health",
            post(health_plugin),
        )
        .route(
            "/v1/tenants/:tenant/plugins/:id/update",
            post(update_plugin),
        )
        .route(
            "/v1/tenants/:tenant/plugins/:id/rollback",
            post(rollback_plugin),
        )
        .route(
            "/v1/tenants/:tenant/plugins/:id/canary",
            post(canary_plugin),
        )
        .route(
            "/v1/tenants/:tenant/plugins/:id/revoke",
            post(revoke_plugin),
        )
}

/// Extract a JSON string array field into a `Vec<String>`, dropping non-string entries.
/// Shared by the catalog builder and the installed-plugin lifecycle.
fn string_array(item: &Value, key: &str) -> Vec<String> {
    item.get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default()
}
