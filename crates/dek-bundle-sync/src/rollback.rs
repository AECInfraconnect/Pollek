// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackState {
    pub tenant_id: String,
    pub device_id: String,
    pub highest_bundle_generation: u64,
    pub highest_root_version: u64,
    pub highest_snapshot_version: u64,
    pub highest_timestamp_version: u64,
    pub accepted_key_ids: Vec<String>,
    pub last_accepted_at: DateTime<Utc>,
    pub last_known_good_bundle_id: String,
}

impl Default for RollbackState {
    fn default() -> Self {
        Self {
            tenant_id: String::new(),
            device_id: String::new(),
            highest_bundle_generation: 0,
            highest_root_version: 0,
            highest_snapshot_version: 0,
            highest_timestamp_version: 0,
            accepted_key_ids: vec![],
            last_accepted_at: Utc::now(),
            last_known_good_bundle_id: String::new(),
        }
    }
}

pub struct RollbackManager {
    state_path: PathBuf,
}

impl RollbackManager {
    pub fn new(data_dir: &std::path::Path) -> Self {
        let state_path = data_dir
            .join("state")
            .join("security")
            .join("rollback_state.json");
        Self { state_path }
    }

    pub fn load(&self) -> Result<RollbackState> {
        if !self.state_path.exists() {
            return Ok(RollbackState::default());
        }
        let content = fs::read_to_string(&self.state_path)
            .with_context(|| format!("Failed to read rollback state at {:?}", self.state_path))?;
        let state: RollbackState =
            serde_json::from_str(&content).context("Failed to parse rollback state JSON")?;
        Ok(state)
    }

    pub fn save(&self, state: &RollbackState) -> Result<()> {
        if let Some(parent) = self.state_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(state)?;
        fs::write(&self.state_path, content)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&self.state_path, fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    pub fn check_and_update_tuf(
        &self,
        tenant_id: &str,
        device_id: &str,
        root_version: u64,
        snapshot_version: u64,
        timestamp_version: u64,
    ) -> Result<RollbackState> {
        let mut state = self.load()?;

        // Trust but verify tenant
        if !state.tenant_id.is_empty() && state.tenant_id != tenant_id {
            return Err(anyhow::anyhow!(
                "Rollback violation: Tenant ID mismatch. Expected {}, got {}",
                state.tenant_id,
                tenant_id
            ));
        }
        if !state.device_id.is_empty() && state.device_id != device_id {
            return Err(anyhow::anyhow!(
                "Rollback violation: Device ID mismatch. Expected {}, got {}",
                state.device_id,
                device_id
            ));
        }

        if root_version < state.highest_root_version {
            return Err(anyhow::anyhow!(
                "Rollback violation: root version {} is older than known {}",
                root_version,
                state.highest_root_version
            ));
        }
        if snapshot_version < state.highest_snapshot_version {
            return Err(anyhow::anyhow!(
                "Rollback violation: snapshot version {} is older than known {}",
                snapshot_version,
                state.highest_snapshot_version
            ));
        }
        if timestamp_version < state.highest_timestamp_version {
            return Err(anyhow::anyhow!(
                "Rollback violation: timestamp version {} is older than known {}",
                timestamp_version,
                state.highest_timestamp_version
            ));
        }

        state.tenant_id = tenant_id.to_string();
        state.device_id = device_id.to_string();
        state.highest_root_version = root_version;
        state.highest_snapshot_version = snapshot_version;
        state.highest_timestamp_version = timestamp_version;
        state.last_accepted_at = Utc::now();

        self.save(&state)?;
        Ok(state)
    }

    pub fn mark_last_known_good(&self, bundle_id: &str) -> Result<()> {
        let mut state = self.load()?;
        state.last_known_good_bundle_id = bundle_id.to_string();
        self.save(&state)?;
        Ok(())
    }

    pub fn revert_to_lkg(&self, data_dir: &std::path::Path) -> Result<String> {
        let state = self.load()?;
        if state.last_known_good_bundle_id.is_empty() {
            return Err(anyhow::anyhow!(
                "No last known good bundle available to revert to"
            ));
        }

        let lkg_id = &state.last_known_good_bundle_id;
        let target_dir = data_dir.join("state").join("bundles");
        let bundle_dir = target_dir.join(format!("bundle_{}", lkg_id));
        let latest_symlink = target_dir.join("latest");

        if !bundle_dir.exists() {
            return Err(anyhow::anyhow!(
                "LKG bundle directory does not exist: {:?}",
                bundle_dir
            ));
        }

        let _ = std::fs::remove_file(&latest_symlink);
        #[cfg(unix)]
        std::os::unix::fs::symlink(&bundle_dir, &latest_symlink)?;
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&bundle_dir, &latest_symlink)?;

        Ok(lkg_id.clone())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use std::env;

    #[test]
    fn test_downgrade_attack_prevented() {
        let dir = env::temp_dir().join(format!(
            "rollback_test_{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let manager = RollbackManager::new(&dir);

        // Initial success
        let res = manager.check_and_update_tuf("t1", "d1", 10, 10, 10);
        assert!(res.is_ok());

        // Same version success
        let res = manager.check_and_update_tuf("t1", "d1", 10, 10, 10);
        assert!(res.is_ok());

        // Downgrade root
        let res = manager.check_and_update_tuf("t1", "d1", 9, 10, 10);
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .to_string()
            .contains("Rollback violation: root version 9 is older than known 10"));

        // Downgrade snapshot
        let res = manager.check_and_update_tuf("t1", "d1", 10, 9, 10);
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .to_string()
            .contains("Rollback violation: snapshot version 9 is older than known 10"));

        // Downgrade timestamp
        let res = manager.check_and_update_tuf("t1", "d1", 10, 10, 9);
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .to_string()
            .contains("Rollback violation: timestamp version 9 is older than known 10"));

        // Tenant change prevention
        let res = manager.check_and_update_tuf("t2", "d1", 11, 11, 11);
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .to_string()
            .contains("Rollback violation: Tenant ID mismatch"));

        std::fs::remove_dir_all(dir).unwrap_or(());
    }
}
