use async_trait::async_trait;
use dek_domain_schema::deployment_session::LocalizedText;

pub struct WarmCheckCtx {
    // Basic context for warm checks
}

pub enum WarmCheckResult {
    Ok,
    Degraded { reason: LocalizedText },
    Failed { reason: LocalizedText },
}

#[async_trait]
pub trait WarmCheck: Send + Sync {
    fn method_id(&self) -> &str;
    async fn verify(&self, ctx: &WarmCheckCtx) -> WarmCheckResult;
}
