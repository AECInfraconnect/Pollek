use std::path::PathBuf;

pub fn get_known_config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Windows APPDATA & USERPROFILE
    if let Ok(appdata) = std::env::var("APPDATA") {
        let mut claude = PathBuf::from(&appdata);
        claude.push("Claude");
        claude.push("claude_desktop_config.json");
        paths.push(claude);

        let mut vscode = PathBuf::from(&appdata);
        vscode.push("Code");
        vscode.push("User");
        vscode.push("globalStorage");
        vscode.push("rooveterinaryinc.roo-cline");
        vscode.push("settings");
        vscode.push("cline_mcp_settings.json");
        paths.push(vscode);
    }

    if let Ok(userprofile) = std::env::var("USERPROFILE") {
        let mut cursor = PathBuf::from(&userprofile);
        cursor.push(".cursor");
        cursor.push("mcp.json");
        paths.push(cursor);

        let mut windsurf = PathBuf::from(&userprofile);
        windsurf.push(".codeium");
        windsurf.push("windsurf");
        windsurf.push("mcp_config.json");
        paths.push(windsurf);
    }

    // HOME for macOS and Linux
    if let Ok(home) = std::env::var("HOME") {
        // macOS
        let mut mac_claude = PathBuf::from(&home);
        mac_claude.push("Library");
        mac_claude.push("Application Support");
        mac_claude.push("Claude");
        mac_claude.push("claude_desktop_config.json");
        paths.push(mac_claude);

        // Linux / generic .config
        let mut linux_claude = PathBuf::from(&home);
        linux_claude.push(".config");
        linux_claude.push("Claude");
        linux_claude.push("claude_desktop_config.json");
        paths.push(linux_claude);

        let mut cursor = PathBuf::from(&home);
        cursor.push(".cursor");
        cursor.push("mcp.json");
        paths.push(cursor);

        let mut windsurf = PathBuf::from(&home);
        windsurf.push(".codeium");
        windsurf.push("windsurf");
        windsurf.push("mcp_config.json");
        paths.push(windsurf);
    }

    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_paths_does_not_panic() {
        let paths = get_known_config_paths();
        assert!(!paths.is_empty());
    }
}
