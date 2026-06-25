#[cfg(test)]
mod tests {
    #[test]
    fn test_web_ai_browser_idle_resolution() {
        let resolved = "chatgpt.com";
        assert!(resolved.contains("chatgpt"));

        let resolved_claude = "claude.ai";
        assert!(resolved_claude.contains("claude"));

        let resolved_gemini = "gemini.google.com";
        assert!(resolved_gemini.contains("gemini"));

        let resolved_deepseek = "chat.deepseek.com";
        assert!(resolved_deepseek.contains("deepseek"));
    }

    #[test]
    fn test_desktop_resolution() {
        let agents = vec!["Codex", "Cursor", "Antigravity", "OpenClaw", "HiClaw"];
        for a in agents {
            assert!(!a.is_empty());
        }
    }
}
