// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use super::ServiceManager;
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Command;

pub struct OsServiceManager {
    service_name: &'static str,
}

impl OsServiceManager {
    pub fn new() -> Self {
        Self {
            service_name: "PollekDEK",
        }
    }

    fn core_exe_path() -> Result<PathBuf> {
        let exe = std::env::current_exe()?;
        let mut dir = exe.parent().context("No parent dir for exe")?.to_path_buf();
        dir.push("dek-core.exe");
        if !dir.exists() {
            anyhow::bail!("dek-core executable not found at {:?}", dir);
        }
        Ok(dir)
    }
}

impl ServiceManager for OsServiceManager {
    fn install(&self) -> Result<()> {
        let exe_path = Self::core_exe_path()?;

        // To bypass OneDrive/UserProfile permission issues for NetworkService,
        // we copy the executable to %ProgramData%\PollekDEK\bin\
        let program_data_env =
            std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
        let root_dir = PathBuf::from(&program_data_env).join("PollekDEK");
        let bin_dir = root_dir.join("bin");
        std::fs::create_dir_all(&bin_dir)?;
        let target_exe = bin_dir.join("dek-core.exe");
        std::fs::copy(&exe_path, &target_exe)?;

        let _ = Command::new("icacls")
            .args([
                root_dir.to_str().unwrap_or_default(),
                "/grant",
                "*S-1-5-20:(OI)(CI)RX",
                "/T",
            ])
            .output();

        let output = Command::new("sc")
            .args([
                "create",
                self.service_name,
                &format!("binPath=\"{}\"", target_exe.display()),
                "start=auto",
                // "NT AUTHORITY\\NetworkService" runs without admin rights but has network access
                "obj=NT AUTHORITY\\NetworkService",
                "DisplayName=Pollek DEK Core",
            ])
            .output()
            .context("Failed to run sc create")?;

        if !output.status.success() {
            anyhow::bail!(
                "Failed to create service: {}",
                String::from_utf8_lossy(&output.stdout)
            );
        }

        Command::new("sc")
            .args([
                "description",
                self.service_name,
                "Pollek DEK IPC Supervisor and Policy Enforcer",
            ])
            .output()?;

        // Generate rollback script
        let rollback_bat_path = bin_dir.join("rollback.bat");
        let bat_content = format!(
            r#"@echo off
if exist "{exe}.bak" (
    copy /Y "{exe}.bak" "{exe}"
)
if exist "{cfg}\update_pending.json" (
    del /F /Q "{cfg}\update_pending.json"
)
sc start PollekDEK
"#,
            exe = target_exe.display(),
            cfg = "C:\\ProgramData\\PollekDEK"
        );
        std::fs::write(&rollback_bat_path, bat_content)?;

        // Configure SC failure actions: Restart after 1st and 2nd failure, run rollback.bat on 3rd failure
        let failure_command = format!("\"{}\"", rollback_bat_path.display());
        Command::new("sc")
            .args([
                "failure",
                self.service_name,
                "reset=",
                "60",
                "actions=",
                "restart/5000/restart/5000/run/5000",
                "command=",
                &failure_command,
            ])
            .output()?;

        Ok(())
    }

    fn uninstall(&self) -> Result<()> {
        let _ = self.stop();
        let output = Command::new("sc")
            .args(["delete", self.service_name])
            .output()
            .context("Failed to run sc delete")?;

        if !output.status.success() {
            anyhow::bail!(
                "Failed to delete service: {}",
                String::from_utf8_lossy(&output.stdout)
            );
        }
        Ok(())
    }

    fn start(&self) -> Result<()> {
        let output = Command::new("sc")
            .args(["start", self.service_name])
            .output()
            .context("Failed to start service")?;
        if !output.status.success() {
            anyhow::bail!(
                "Failed to start service: {}",
                String::from_utf8_lossy(&output.stdout)
            );
        }
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        let output = Command::new("sc")
            .args(["stop", self.service_name])
            .output()
            .context("Failed to stop service")?;
        if !output.status.success() {
            // Note: If already stopped, sc stop returns an error, we can ignore or return it
        }
        Ok(())
    }

    fn status(&self) -> Result<String> {
        let output = Command::new("sc")
            .args(["query", self.service_name])
            .output()
            .context("Failed to get status")?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
