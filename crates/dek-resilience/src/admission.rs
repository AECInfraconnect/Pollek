//! admission.rs — global + per-tenant concurrency limiting (backpressure).
//!
//! Bounds in-flight requests so a burst (or one noisy tenant) can't exhaust
//! memory/threads or starve other tenants. Over the limit => `None` => the
//! caller FAILS CLOSED (deny + 503). Permits release on drop.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub struct AdmissionControl {
    global: Arc<Semaphore>,
    per_tenant_limit: usize,
    tenants: Mutex<HashMap<String, Arc<Semaphore>>>,
}

/// Held for the lifetime of a request; releases both permits on drop.
pub struct AdmitPermit {
    _global: OwnedSemaphorePermit,
    _tenant: OwnedSemaphorePermit,
}

impl AdmissionControl {
    pub fn new(global_limit: usize, per_tenant_limit: usize) -> Arc<Self> {
        Arc::new(Self {
            global: Arc::new(Semaphore::new(global_limit.max(1))),
            per_tenant_limit: per_tenant_limit.max(1),
            tenants: Mutex::new(HashMap::new()),
        })
    }

    fn tenant_sem(&self, tenant: &str) -> Arc<Semaphore> {
        let mut map = self.tenants.lock().unwrap();
        map.entry(tenant.to_string())
            .or_insert_with(|| Arc::new(Semaphore::new(self.per_tenant_limit)))
            .clone()
    }

    /// Non-blocking admission. Returns None if EITHER the global or the tenant
    /// limit is saturated (caller must deny). Acquires global first, then tenant;
    /// on tenant failure the global permit drops automatically.
    pub fn try_admit(&self, tenant: &str) -> Option<AdmitPermit> {
        let g = match self.global.clone().try_acquire_owned() {
            Ok(p) => p,
            Err(_) => {
                metrics::counter!("dek_admission_rejected_total", "scope" => "global").increment(1);
                return None;
            }
        };
        let ts = self.tenant_sem(tenant);
        let t = match ts.try_acquire_owned() {
            Ok(p) => p,
            Err(_) => {
                metrics::counter!("dek_admission_rejected_total",
                    "scope" => "tenant", "tenant" => tenant.to_string())
                .increment(1);
                return None; // `g` drops here, releasing the global permit
            }
        };
        Some(AdmitPermit {
            _global: g,
            _tenant: t,
        })
    }

    pub fn available_global(&self) -> usize {
        self.global.available_permits()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_limit_blocks_when_full() {
        let ac = AdmissionControl::new(2, 10);
        let p1 = ac.try_admit("t1");
        let p2 = ac.try_admit("t2");
        assert!(p1.is_some() && p2.is_some());
        assert!(ac.try_admit("t3").is_none(), "global full -> reject");
        drop(p1);
        assert!(ac.try_admit("t3").is_some(), "permit freed -> admit");
    }

    #[test]
    fn per_tenant_limit_isolates_tenants() {
        let ac = AdmissionControl::new(100, 1);
        let a1 = ac.try_admit("tenant-a");
        assert!(a1.is_some());
        assert!(ac.try_admit("tenant-a").is_none(), "tenant-a at its limit");
        // other tenant unaffected (fairness)
        assert!(ac.try_admit("tenant-b").is_some(), "tenant-b isolated");
    }
}
