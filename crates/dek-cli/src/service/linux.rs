// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use super::ServiceManager;
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub struct OsServiceManager {
    service_name: &'static str,
    unit_path: PathBuf,
}

impl OsServiceManager {
    pub fn new() -> Self {
        Self {
            service_name: "pollek-dek",
            unit_path: PathBuf::from("/etc/systemd/system/pollek-dek.service"),
        }
    }

    fn core_exe_path() -> Result<PathBuf> {
        let exe = std::env::current_exe()?;
        let mut dir = exe.parent().context("No parent dir for exe")?.to_path_buf();
        dir.push("dek-core");
        if !dir.exists() {
            anyhow::bail!("dek-core executable not found at {:?}", dir);
        }
        Ok(dir)
    }
}

impl ServiceManager for OsServiceManager {
    fn install(&self) -> Result<()> {
        let exe_path = Self::core_exe_path()?;
        let unit_content = format!(
            r#"[Unit]
Description=Pollek DEK Core
After=network.target
StartLimitIntervalSec=60
StartLimitBurst=3

[Service]
Type=notify
ExecStart={}
Restart=on-failure
NoNewPrivileges=true
ProtectSystem=full
User=pollekdek
Group=pollekdek
AmbientCapabilities=CAP_BPF CAP_NET_ADMIN
WatchdogSec=30
OnFailure=pollek-dek-rollback.service

[Install]
WantedBy=multi-user.target
"#,
            exe_path.display()
        );

        let rollback_unit_content = format!(
            r#"[Unit]
Description=Pollek DEK Core Rollback

[Service]
Type=oneshot
ExecStart=/bin/sh -c 'if [ -f {exe_path}.bak ]; then mv {exe_path}.bak {exe_path}; fi; rm -f /etc/pollek-dek/update_pending.json; systemctl restart pollek-dek.service'
"#,
            exe_path = exe_path.display()
        );

        fs::write(&self.unit_path, unit_content)?;
        fs::write(
            "/etc/systemd/system/pollek-dek-rollback.service",
            rollback_unit_content,
        )?;
        Command::new("systemctl")
            .arg("daemon-reload")
            .status()
            .context("Failed to reload systemd daemon")?;
        Command::new("systemctl")
            .args(["enable", self.service_name])
            .status()
            .context("Failed to enable service")?;

        Ok(())
    }

    fn uninstall(&self) -> Result<()> {
        let _ = self.stop();
        Command::new("systemctl")
            .args(["disable", self.service_name])
            .status()
            .context("Failed to disable service")?;
        if self.unit_path.exists() {
            fs::remove_file(&self.unit_path)?;
        }
        Command::new("systemctl")
            .arg("daemon-reload")
            .status()
            .context("Failed to reload systemd daemon")?;
        Ok(())
    }

    fn start(&self) -> Result<()> {
        Command::new("systemctl")
            .args(["start", self.service_name])
            .status()
            .context("Failed to start service")?;
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        Command::new("systemctl")
            .args(["stop", self.service_name])
            .status()
            .context("Failed to stop service")?;
        Ok(())
    }

    fn status(&self) -> Result<String> {
        let output = Command::new("systemctl")
            .args(["status", self.service_name])
            .output()
            .context("Failed to get status")?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
