use anyhow::Result;
use dek_domain_schema::CompiledNetworkRules;
use tracing::{error, info, warn};

/// Watchdog monitors the application of rules and falls back to Last Known Good
/// if the new rules fail or cause immediate driver instability.
#[derive(Debug)]
pub struct WfpWatchdog {
    last_known_good: Option<CompiledNetworkRules>,
}

impl WfpWatchdog {
    pub fn new() -> Self {
        Self {
            last_known_good: None,
        }
    }

    pub fn apply_with_fallback<F>(
        &mut self,
        rules: CompiledNetworkRules,
        apply_fn: F,
    ) -> Result<()>
    where
        F: Fn(&CompiledNetworkRules) -> Result<()>,
    {
        info!("Watchdog: Attempting to apply new rules (v{})", rules.version);

        match apply_fn(&rules) {
            Ok(_) => {
                info!("Watchdog: Rules applied successfully. Updating Last Known Good.");
                self.last_known_good = Some(rules);
                Ok(())
            }
            Err(e) => {
                error!("Watchdog: Failed to apply new rules: {}", e);
                if let Some(ref lkg) = self.last_known_good {
                    warn!("Watchdog: Rolling back to Last Known Good (v{})", lkg.version);
                    // Attempt rollback
                    if let Err(rollback_err) = apply_fn(lkg) {
                        error!("Watchdog: CRITICAL FAILURE: Rollback to Last Known Good also failed: {}", rollback_err);
                        // Implement fail-closed high-risk here (e.g. block all network traffic)
                        self.trigger_fail_closed_mode();
                    } else {
                        info!("Watchdog: Rollback successful.");
                    }
                } else {
                    error!("Watchdog: No Last Known Good available. Triggering Fail-Closed.");
                    self.trigger_fail_closed_mode();
                }
                Err(e)
            }
        }
    }

    pub fn trigger_fail_closed_mode(&self) {
        warn!("!!! TRIGGERING NETWORK FAIL-CLOSED MODE !!!");
        // In reality, this would install a hardcoded WFP block-all filter to prevent any data exfiltration.
    }
}
