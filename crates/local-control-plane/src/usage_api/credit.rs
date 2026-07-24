//! Provider credit ledger: persisted per-provider credit config and the
//! honest consumed/remaining credit-status computation over real spend.

use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct ProviderCreditConfigV1 {
    pub(super) provider: String,
    /// Value of one credit in the account currency (e.g. 0.001 => $10 buys
    /// 10,000 credits). Must be > 0 to contribute credits.
    pub(super) currency_per_credit: f64,
    /// Optional prepaid balance, in credits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) initial_credits: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct CreditLedgerConfigV1 {
    #[serde(default = "credit_ledger_schema")]
    pub(super) schema_version: String,
    #[serde(default = "default_currency")]
    pub(super) currency: String,
    #[serde(default)]
    pub(super) providers: Vec<ProviderCreditConfigV1>,
}

pub(super) fn credit_ledger_schema() -> String {
    "pollek.credit_ledger.v1".to_string()
}

pub(super) fn default_currency() -> String {
    "USD".to_string()
}

impl Default for CreditLedgerConfigV1 {
    fn default() -> Self {
        Self {
            schema_version: credit_ledger_schema(),
            currency: default_currency(),
            providers: Vec::new(),
        }
    }
}

pub(super) fn credit_ledger_path() -> std::path::PathBuf {
    std::path::PathBuf::from("pollek-local-data/provider_credits.v1.json")
}

pub(super) fn load_credit_ledger() -> CreditLedgerConfigV1 {
    std::fs::read_to_string(credit_ledger_path())
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

pub(super) fn save_credit_ledger(config: &CreditLedgerConfigV1) -> std::io::Result<()> {
    let path = credit_ledger_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let body = serde_json::to_string_pretty(config)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    std::fs::write(path, body)
}

/// Normalize provider keys so "OpenAI" configured by the user matches
/// "openai" observed on events.
pub(super) fn credit_provider_key(provider: &str) -> String {
    provider.trim().to_ascii_lowercase()
}

/// Compute per-provider credit consumption from observed cost. Only providers
/// with a positive currency_per_credit contribute credits.
pub(super) fn compute_credit_status(
    config: &CreditLedgerConfigV1,
    cost_by_provider: &[(String, f64)],
) -> Value {
    let cost_lookup: std::collections::HashMap<String, f64> = cost_by_provider
        .iter()
        .map(|(provider, cost)| (credit_provider_key(provider), *cost))
        .collect();

    let mut providers = Vec::new();
    let mut total_consumed = 0.0_f64;
    let mut total_remaining = 0.0_f64;
    let mut has_remaining = false;

    for entry in &config.providers {
        if entry.currency_per_credit <= 0.0 {
            continue;
        }
        let consumed_cost = cost_lookup
            .get(&credit_provider_key(&entry.provider))
            .copied()
            .unwrap_or(0.0);
        let consumed_credits = consumed_cost / entry.currency_per_credit;
        total_consumed += consumed_credits;
        let remaining_credits = entry
            .initial_credits
            .map(|initial| initial - consumed_credits);
        if let Some(remaining) = remaining_credits {
            total_remaining += remaining;
            has_remaining = true;
        }
        providers.push(json!({
            "provider": entry.provider,
            "label": entry.label,
            "currency_per_credit": entry.currency_per_credit,
            "initial_credits": entry.initial_credits,
            "consumed_cost": consumed_cost,
            "consumed_credits": consumed_credits,
            "remaining_credits": remaining_credits,
        }));
    }

    json!({
        "currency": config.currency,
        "providers": providers,
        "total_consumed_credits": total_consumed,
        "total_remaining_credits": if has_remaining { Some(total_remaining) } else { None },
    })
}

pub(super) async fn get_credit_ledger(
    State(state): State<AppState>,
    Path(tenant): Path<String>,
    Query(params): Query<UsageSummaryParams>,
) -> impl IntoResponse {
    let config = load_credit_ledger();
    let query = summary_query(tenant, params);
    let cost_by_provider = match state.observability_store.ai_usage_summary(query).await {
        Ok(summary) => summary
            .by_provider
            .into_iter()
            .map(|row| (row.label, row.total_cost))
            .collect::<Vec<_>>(),
        Err(_) => Vec::new(),
    };
    let status = compute_credit_status(&config, &cost_by_provider);
    (
        StatusCode::OK,
        Json(json!({ "config": config, "status": status })),
    )
}

pub(super) async fn put_credit_ledger(
    Path(_tenant): Path<String>,
    Json(mut config): Json<CreditLedgerConfigV1>,
) -> impl IntoResponse {
    if config.schema_version.is_empty() {
        config.schema_version = credit_ledger_schema();
    }
    if config.currency.is_empty() {
        config.currency = default_currency();
    }
    match save_credit_ledger(&config) {
        Ok(()) => (StatusCode::OK, Json(json!({ "config": config }))),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": err.to_string() })),
        ),
    }
}
