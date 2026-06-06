use anyhow::{Context, Result};
use std::process::Command;

pub fn enable() -> Result<()> {
    // Enable proxy and set server to 127.0.0.1:43890 via registry
    Command::new("reg")
        .args(["add", "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings", "/v", "ProxyEnable", "/t", "REG_DWORD", "/d", "1", "/f"])
        .output()
        .context("Failed to enable proxy via registry")?;

    Command::new("reg")
        .args(["add", "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings", "/v", "ProxyServer", "/t", "REG_SZ", "/d", "127.0.0.1:43890", "/f"])
        .output()
        .context("Failed to set proxy server via registry")?;

    Ok(())
}

pub fn disable() -> Result<()> {
    Command::new("reg")
        .args(["add", "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings", "/v", "ProxyEnable", "/t", "REG_DWORD", "/d", "0", "/f"])
        .output()
        .context("Failed to disable proxy via registry")?;

    Ok(())
}
