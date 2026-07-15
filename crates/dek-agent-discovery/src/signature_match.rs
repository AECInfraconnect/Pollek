use dek_fingerprint_defs::model::{AgentSignatureV2, InstalledAppSignatureDef};

pub struct ProcessFacts<'a> {
    pub process_name: &'a str,
    pub exe_path: &'a str,
    pub cmdline: &'a str,
    pub installed_paths: &'a [String],
}

pub struct SignatureMatch {
    pub id: String,
    pub display_name: String,
    pub vendor: Option<String>,
    pub agent_type: String,
    pub confidence: f64,
    pub matched_by: &'static str,
    pub capability_tags: Vec<String>,
}

/// Interpreter/runtime host processes that many different agents run under.
/// A signature listing one of these as a `process_name` must never match on
/// the bare process name alone — that would tag every `node`/`python` on the
/// machine as that agent (false positives and duplicate candidates). Such
/// signatures still match through cmd patterns, exe paths, and markers.
pub(crate) const GENERIC_HOST_PROCESSES: &[&str] = &[
    "node",
    "node.exe",
    "python",
    "python.exe",
    "python3",
    "python3.exe",
    "bun",
    "bun.exe",
    "deno",
    "deno.exe",
    "java",
    "java.exe",
    "dotnet",
    "dotnet.exe",
];

pub(crate) fn is_generic_host_process(name: &str) -> bool {
    let name = name.trim().to_ascii_lowercase();
    GENERIC_HOST_PROCESSES.contains(&name.as_str())
}

pub fn match_process(
    facts: &ProcessFacts,
    sigs: &[AgentSignatureV2],
    apps: &[InstalledAppSignatureDef],
) -> Option<SignatureMatch> {
    let mut best: Option<SignatureMatch> = None;
    let pname = facts.process_name.to_lowercase();
    let exe = facts.exe_path.replace('\\', "/").to_lowercase();
    let cmd = facts.cmdline.to_lowercase();

    for s in sigs {
        let mut conf = 0.0f64;
        let mut by = "";

        if !pname.trim().is_empty()
            && !is_generic_host_process(&pname)
            && s.process_names.iter().any(|n| {
                !n.trim().is_empty()
                    && !is_generic_host_process(n)
                    && (n.eq_ignore_ascii_case(&pname)
                        || strip_ext(&pname) == strip_ext(&n.to_lowercase()))
            })
        {
            conf = conf.max(0.9);
            by = "process_name";
        }

        if !exe.is_empty()
            && s.exe_path_patterns
                .iter()
                .any(|p| !p.trim().is_empty() && glob_match(&p.to_lowercase(), &exe))
        {
            conf = conf.max(0.95);
            by = "exe_path";
        }

        if !cmd.is_empty()
            && s.cmd_patterns
                .iter()
                .any(|p| !p.trim().is_empty() && glob_match(&p.to_lowercase(), &cmd))
        {
            conf = conf.max(0.85);
            by = "cmd_pattern";
        }

        if (!exe.trim().is_empty() || !cmd.trim().is_empty())
            && s.cli_binaries.iter().any(|b| {
                !b.trim().is_empty()
                    && (basename(&exe) == *b || cmd.split_whitespace().next() == Some(b))
            })
        {
            conf = conf.max(0.8);
            by = "cli_binary";
        }

        if s.install_markers.iter().any(|m| {
            facts.installed_paths.iter().any(|ip| {
                !m.path.trim().is_empty()
                    && glob_match(
                        &m.path.to_lowercase(),
                        &ip.replace('\\', "/").to_lowercase(),
                    )
            })
        }) {
            conf = conf.max(0.85);
            by = "install_marker";
        }

        if conf > 0.0 && best.as_ref().map(|b| conf > b.confidence).unwrap_or(true) {
            best = Some(SignatureMatch {
                id: s.id.clone(),
                display_name: s.display_name.clone(),
                vendor: s.vendor.clone(),
                agent_type: s.agent_type.clone(),
                confidence: conf,
                matched_by: leak(by),
                capability_tags: s.capability_tags.clone(),
            });
        }
    }

    for a in apps {
        let hit_path = a.markers.iter().any(|m| {
            m.paths
                .iter()
                .any(|path| !exe.is_empty() && glob_match(&path.to_lowercase(), &exe))
        });
        let hit_name = a.process_names().iter().any(|n| {
            !pname.trim().is_empty() && !n.trim().is_empty() && n.eq_ignore_ascii_case(&pname)
        });
        if hit_path || hit_name {
            let conf = if hit_path { 0.95 } else { 0.9 };
            if best.as_ref().map(|b| conf > b.confidence).unwrap_or(true) {
                best = Some(SignatureMatch {
                    id: a.id.clone(),
                    display_name: a.name.clone(),
                    vendor: Some(a.vendor.clone()),
                    agent_type: a.agent_type.clone(),
                    confidence: conf,
                    matched_by: if hit_path {
                        "install_path"
                    } else {
                        "process_name"
                    },
                    capability_tags: a.capability_tags.clone(),
                });
            }
        }
    }
    best
}

fn strip_ext(s: &str) -> String {
    s.rsplit_once('.')
        .map(|(a, _)| a.to_string())
        .unwrap_or_else(|| s.to_string())
}
fn basename(path: &str) -> String {
    path.rsplit('/').next().map(strip_ext).unwrap_or_default()
}
fn leak(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

pub fn glob_match(pat: &str, text: &str) -> bool {
    let pat = pat
        .replace("**", "\u{1}")
        .replace('*', "\u{2}")
        .replace('\u{1}', "*");
    fn rec(p: &[u8], t: &[u8]) -> bool {
        match p.first() {
            None => t.is_empty(),
            Some(b'*') => rec(&p[1..], t) || (!t.is_empty() && rec(p, &t[1..])),
            Some(&0x02) => (!t.is_empty() && t[0] != b'/' && rec(p, &t[1..])) || rec(&p[1..], t),
            Some(&c) => !t.is_empty() && t[0] == c && rec(&p[1..], &t[1..]),
        }
    }
    rec(pat.as_bytes(), text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_handles_windowsapps_codex() {
        assert!(glob_match("**/windowsapps/openai.codex_*/**",
            "c:/program files/windowsapps/openai.codex_26.616.3767.0_x64__2p2nqsd0c76g0/app/codex.exe"));
    }

    fn baseline_match(process_name: &str, exe_path: &str, cmdline: &str) -> Option<SignatureMatch> {
        let baseline = dek_fingerprint_defs::embedded_baseline();
        let facts = ProcessFacts {
            process_name,
            exe_path,
            cmdline,
            installed_paths: &[],
        };
        match_process(
            &facts,
            &baseline.signatures,
            &baseline.installed_app_signatures,
        )
    }

    #[test]
    fn vllm_matches_from_real_launch_cmdline() {
        let m = baseline_match(
            "python3",
            "/usr/bin/python3",
            "python3 -m vllm.entrypoints.openai.api_server --model meta-llama/Llama-3-8b",
        );
        assert_eq!(m.map(|m| m.id), Some("vllm".to_string()));

        let m2 = baseline_match("python3", "/usr/bin/python3", "vllm serve Qwen/Qwen3-8B");
        assert_eq!(m2.map(|m| m.id), Some("vllm".to_string()));
    }

    #[test]
    fn sglang_matches_from_launch_server_cmdline() {
        let m = baseline_match(
            "python3",
            "/usr/bin/python3",
            "python3 -m sglang.launch_server --model-path Qwen/Qwen3-8B --port 30000",
        );
        assert_eq!(m.map(|m| m.id), Some("sglang".to_string()));
    }

    #[test]
    fn claw_family_legacy_installs_match_openclaw() {
        // Legacy Clawdbot install (pre-rename) still running under node.
        let clawdbot = baseline_match(
            "node",
            "/usr/local/bin/node",
            "node /usr/local/lib/node_modules/clawdbot/dist/index.js gateway",
        );
        assert_eq!(clawdbot.map(|m| m.id), Some("openclaw".to_string()));

        // Moltbot-era install.
        let moltbot = baseline_match(
            "node",
            "/usr/local/bin/node",
            "node /home/user/.moltbot/gateway.js",
        );
        assert_eq!(moltbot.map(|m| m.id), Some("openclaw".to_string()));

        // Current OpenClaw gateway.
        let openclaw = baseline_match(
            "node",
            "/usr/local/bin/node",
            "node /usr/local/lib/node_modules/openclaw/dist/gateway.js",
        );
        assert_eq!(openclaw.map(|m| m.id), Some("openclaw".to_string()));
    }

    #[test]
    fn headless_cdp_browser_matches_blackbox_automation() {
        // A CDP-driven Chromium (Playwright/Puppeteer/browser-use style).
        let m = baseline_match(
            "chromium",
            "/usr/bin/chromium",
            "/usr/bin/chromium --headless --remote-debugging-port=9222 --user-data-dir=/tmp/pw",
        );
        assert_eq!(
            m.map(|m| m.id),
            Some("headless_browser_automation".to_string())
        );

        // The headless-only shell binary matches by process name alone.
        let shell = baseline_match(
            "headless_shell",
            "/opt/pw-browsers/chromium/headless_shell",
            "",
        );
        assert_eq!(
            shell.map(|m| m.id),
            Some("headless_browser_automation".to_string())
        );
    }

    #[test]
    fn browser_use_agent_matches_from_python_cmdline() {
        let m = baseline_match(
            "python3",
            "/usr/bin/python3",
            "python3 -c import browser_use; agent.run()",
        );
        assert_eq!(m.map(|m| m.id), Some("browser_use_agent".to_string()));
    }

    #[test]
    fn agentic_browsers_match_by_process_name() {
        let comet = baseline_match("Comet", "/applications/comet.app/contents/macos/comet", "");
        assert_eq!(comet.map(|m| m.id), Some("comet_browser".to_string()));

        let atlas = baseline_match(
            "ChatGPT Atlas",
            "/applications/chatgpt atlas.app/contents/macos/chatgpt atlas",
            "",
        );
        assert_eq!(
            atlas.map(|m| m.id),
            Some("chatgpt_atlas_browser".to_string())
        );

        let dia = baseline_match("Dia", "/applications/dia.app/contents/macos/dia", "");
        assert_eq!(dia.map(|m| m.id), Some("dia_browser".to_string()));
    }

    #[test]
    fn bare_interpreter_processes_never_match_agent_signatures() {
        // node/python with no agent-specific cmdline must not be tagged as
        // OpenClaw/HiClaw/etc. (that caused false positives + duplicates).
        for (proc_name, exe) in [
            ("node", "/usr/local/bin/node"),
            ("node.exe", "c:/program files/nodejs/node.exe"),
            ("python3", "/usr/bin/python3"),
            ("bun", "/usr/local/bin/bun"),
        ] {
            let m = baseline_match(proc_name, exe, "");
            assert!(
                m.is_none(),
                "bare interpreter {proc_name} must not match, got {:?}",
                m.map(|m| m.id)
            );
        }
    }

    #[test]
    fn new_local_engines_match_by_process_name() {
        for (proc_name, expected) in [
            ("text-generation-launcher", "tgi"),
            ("xinference-local", "xinference"),
            ("llamafile", "llamafile"),
            ("AnythingLLM", "anythingllm"),
            ("Msty", "msty"),
        ] {
            let m = baseline_match(proc_name, "", "");
            assert_eq!(
                m.map(|m| m.id),
                Some(expected.to_string()),
                "{proc_name} should match {expected}"
            );
        }
    }
}
