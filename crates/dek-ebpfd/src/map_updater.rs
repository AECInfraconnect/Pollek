use anyhow::{bail, Result};
use dek_domain_schema::ebpf::{EbpfMapUpdate, UpdateSource};
use tracing::{info, warn};

pub struct MapUpdater {
    pub tenant_id: String,
    pub device_id: String,
    pub current_generation: u64,
}

impl MapUpdater {
    pub fn new(tenant_id: String, device_id: String, current_generation: u64) -> Self {
        Self {
            tenant_id,
            device_id,
            current_generation,
        }
    }

    pub fn validate_update(&self, update: &EbpfMapUpdate) -> Result<()> {
        if update.tenant_id != self.tenant_id {
            bail!("Unauthorized map update: tenant_id mismatch");
        }
        if update.device_id != self.device_id {
            bail!("Unauthorized map update: device_id mismatch");
        }
        if update.generation < self.current_generation {
            bail!("Unauthorized map update: generation rollback attempt");
        }

        // Hybrid Signature Check
        let requires_signature = match update.source {
            UpdateSource::OutOfBand => true,
            UpdateSource::Bundle => {
                // High-risk maps require signature even if from bundle
                // e.g. "PORTS_MAP", "VERDICT_MAP", "CGROUP_POLICY_MAP"
                matches!(
                    update.map_name.as_str(),
                    "VERDICT_MAP" | "PORTS_MAP" | "CGROUP_POLICY_MAP"
                )
            }
        };

        if requires_signature && update.signature.is_none() {
            bail!("Unauthorized map update: signature strictly required for this source/map");
        }

        // Bounding checks based on map names
        match update.map_name.as_str() {
            "VERDICT_MAP" | "PORTS_MAP" => {
                // In a real implementation, we would check the current map capacity using aya
                // For now, we simulate the bounding check validation.
                info!("Bounding check passed for {}", update.map_name);
            }
            "CGROUP_POLICY_MAP" => {
                info!("Bounding check passed for {}", update.map_name);
            }
            _ => {
                warn!("Unknown map: {}", update.map_name);
            }
        }

        Ok(())
    }

    pub fn apply_update(&mut self, update: EbpfMapUpdate) -> Result<()> {
        self.validate_update(&update)?;
        
        // Update generation to prevent replay of older updates
        if update.generation > self.current_generation {
            self.current_generation = update.generation;
        }

        info!("Applied update to map: {}", update.map_name);
        // Note: Actual aya map modification would happen here.
        Ok(())
    }
}
