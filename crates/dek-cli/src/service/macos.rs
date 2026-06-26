// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use super::ServiceManager;
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub struct OsServiceManager {
    service_label: &'static str,
    plist_path: PathBuf,
}

impl OsServiceManager {
    pub fn new() -> Self {
        Self {
            service_label: "com.aecinfraconnect.pollekdek",
            plist_path: PathBuf::from("/Library/LaunchDaemons/com.aecinfraconnect.pollekdek.plist"),
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
        let plist_content = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>UserName</key>
    <string>pollekdek</string>
    <key>StandardOutPath</key>
    <string>/Library/Logs/PollekDEK/stdout.log</string>
    <key>StandardErrorPath</key>
    <string>/Library/Logs/PollekDEK/stderr.log</string>
</dict>
</plist>
"#,
            self.service_label,
            exe_path.display()
        );

        fs::write(&self.plist_path, plist_content)?;
        Command::new("launchctl")
            .args(["load", "-w", &self.plist_path.to_string_lossy()])
            .status()
            .context("Failed to load plist into launchd")?;

        Ok(())
    }

    fn uninstall(&self) -> Result<()> {
        Command::new("launchctl")
            .args(["unload", "-w", &self.plist_path.to_string_lossy()])
            .status()
            .context("Failed to unload plist from launchd")?;
        if self.plist_path.exists() {
            fs::remove_file(&self.plist_path)?;
        }
        Ok(())
    }

    fn start(&self) -> Result<()> {
        Command::new("launchctl")
            .args(["start", self.service_label])
            .status()
            .context("Failed to start service")?;
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        Command::new("launchctl")
            .args(["stop", self.service_label])
            .status()
            .context("Failed to stop service")?;
        Ok(())
    }

    fn status(&self) -> Result<String> {
        let output = Command::new("launchctl")
            .args(["list", self.service_label])
            .output()
            .context("Failed to get status")?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
