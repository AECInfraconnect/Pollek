//! End-to-end tests for the detection engine against the real `core-v1` pack.

use dek_detection::eval::ObservedEvent;
use dek_detection::loader::{sha256_text_lf, verify_and_load_pack, PackManifest};
use dek_detection::{build_coverage, evaluate, glob_match, load_pack_dir};
use std::path::{Path, PathBuf};

fn pack_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts/detections/packs/core-v1")
}

fn opt(value: Option<&str>) -> Option<String> {
    value.map(Into::into)
}

macro_rules! ev {
    (
        $id:expr,
        $ts_ms:expr,
        $activity:expr,
        $action:expr,
        $classification:expr,
        $taint:expr,
        $path:expr,
        $host_rep:expr,
        $in_allowlist:expr $(,)?
    ) => {
        ObservedEvent {
            event_id: $id.into(),
            ts_ms: $ts_ms,
            agent_id: "agent-1".into(),
            session_id: "sess-1".into(),
            activity: $activity.into(),
            action: $action.into(),
            resource_classification: opt($classification),
            provenance_taint: opt($taint),
            path: opt($path),
            host: None,
            host_reputation: opt($host_rep),
            in_allowlist: $in_allowlist,
        }
    };
}

#[test]
fn all_rules_pass_the_coverage_gate() -> anyhow::Result<()> {
    let rules = load_pack_dir(pack_dir())?;
    assert_eq!(rules.len(), 5, "expected 5 core rules");
    for rule in &rules {
        assert!(
            !rule.maps.owasp_agentic.is_empty(),
            "{} missing owasp_agentic",
            rule.id
        );
        assert!(
            !rule.maps.atlas.is_empty() || !rule.maps.attack.is_empty(),
            "{} missing ATLAS/ATT&CK",
            rule.id
        );
        assert!(
            rule.response.observe_only_fallback,
            "{} not observe-safe",
            rule.id
        );
    }
    Ok(())
}

#[test]
fn manifest_integrity_verifies_and_loads() -> anyhow::Result<()> {
    let rules = verify_and_load_pack(pack_dir(), |manifest: &PackManifest, _dir| {
        assert_eq!(manifest.pack_id, "pollek-core");
        Ok(())
    })?;
    assert_eq!(rules.len(), 5);
    Ok(())
}

#[test]
fn glob_matches_secret_paths() {
    assert!(glob_match("**/.env", "/home/u/project/.env"));
    assert!(glob_match("**/.ssh/**", "/home/u/.ssh/id_rsa"));
    assert!(glob_match("**/id_rsa", "/home/u/.ssh/id_rsa"));
    assert!(glob_match("**/.env.*", "/app/.env.production"));
    assert!(!glob_match("**/.env", "/home/u/project/main.ts"));
    assert!(!glob_match("*.ts", "src/main.tsx"));
    assert!(glob_match("*.ts", "main.ts"));
}

#[test]
fn manifest_hashes_are_canonical_across_line_endings() -> anyhow::Result<()> {
    let lf = b"id: POLLEK-DET-0001\nname: Test\n";
    let crlf = b"id: POLLEK-DET-0001\r\nname: Test\r\n";
    assert_eq!(sha256_text_lf(lf)?, sha256_text_lf(crlf)?);
    Ok(())
}

#[test]
fn det0001_fires_on_secret_read() -> anyhow::Result<()> {
    let rules = load_pack_dir(pack_dir())?;
    let det0001 = rules
        .iter()
        .find(|rule| rule.id == "POLLEK-DET-0001")
        .ok_or_else(|| anyhow::anyhow!("POLLEK-DET-0001 missing"))?;
    let events = vec![ev!(
        "e1",
        1000,
        "FileRead",
        "read",
        None,
        None,
        Some("/home/u/app/.env"),
        None,
        false,
    )];
    let hit =
        evaluate(det0001, &events).ok_or_else(|| anyhow::anyhow!("should fire on .env read"))?;
    assert_eq!(hit.matched_event_ids, vec!["e1"]);

    let benign = vec![ev!(
        "e2",
        1000,
        "FileRead",
        "read",
        None,
        None,
        Some("/home/u/app/main.ts"),
        None,
        false,
    )];
    assert!(evaluate(det0001, &benign).is_none());
    Ok(())
}

#[test]
fn det0002_fires_on_sensitive_read_then_new_domain_upload() -> anyhow::Result<()> {
    let rules = load_pack_dir(pack_dir())?;
    let det0002 = rules
        .iter()
        .find(|rule| rule.id == "POLLEK-DET-0002")
        .ok_or_else(|| anyhow::anyhow!("POLLEK-DET-0002 missing"))?;

    let positive = vec![
        ev!(
            "r1",
            1000,
            "FileRead",
            "read",
            Some("sensitive"),
            None,
            Some("/data/customers.csv"),
            None,
            false,
        ),
        ev!(
            "u1",
            1050,
            "WebUpload",
            "upload",
            None,
            None,
            None,
            Some("neutral"),
            false,
        ),
    ];
    let hit =
        evaluate(det0002, &positive).ok_or_else(|| anyhow::anyhow!("sequence should fire"))?;
    assert_eq!(hit.matched_event_ids, vec!["r1", "u1"]);

    let allowlisted = vec![
        ev!(
            "r2",
            1000,
            "FileRead",
            "read",
            Some("sensitive"),
            None,
            Some("/data/customers.csv"),
            None,
            false,
        ),
        ev!(
            "u2",
            1050,
            "WebUpload",
            "upload",
            None,
            None,
            Some("trusted"),
            Some("trusted"),
            true,
        ),
    ];
    assert!(evaluate(det0002, &allowlisted).is_none());

    let too_late = vec![
        ev!(
            "r3",
            1000,
            "FileRead",
            "read",
            Some("sensitive"),
            None,
            Some("/data/x.csv"),
            None,
            false,
        ),
        ev!(
            "u3",
            201_000,
            "WebUpload",
            "upload",
            None,
            None,
            None,
            Some("neutral"),
            false,
        ),
    ];
    assert!(evaluate(det0002, &too_late).is_none());
    Ok(())
}

#[test]
fn det0003_fires_on_tainted_read_then_action() -> anyhow::Result<()> {
    let rules = load_pack_dir(pack_dir())?;
    let det0003 = rules
        .iter()
        .find(|rule| rule.id == "POLLEK-DET-0003")
        .ok_or_else(|| anyhow::anyhow!("POLLEK-DET-0003 missing"))?;
    let events = vec![
        ev!(
            "w1",
            5000,
            "WebVisit",
            "read",
            None,
            Some("web"),
            None,
            None,
            false,
        ),
        ev!(
            "x1",
            5500,
            "ShellCommand",
            "execute",
            None,
            None,
            None,
            None,
            false,
        ),
    ];
    assert!(evaluate(det0003, &events).is_some());

    let trusted = vec![
        ev!(
            "w2",
            5000,
            "WebVisit",
            "read",
            None,
            Some("user_direct"),
            None,
            None,
            false,
        ),
        ev!(
            "x2",
            5500,
            "ShellCommand",
            "execute",
            None,
            None,
            None,
            None,
            false,
        ),
    ];
    assert!(evaluate(det0003, &trusted).is_none());
    Ok(())
}

#[test]
fn det0005_fires_on_call_spike() -> anyhow::Result<()> {
    let rules = load_pack_dir(pack_dir())?;
    let det0005 = rules
        .iter()
        .find(|rule| rule.id == "POLLEK-DET-0005")
        .ok_or_else(|| anyhow::anyhow!("POLLEK-DET-0005 missing"))?;
    let mut events = Vec::new();
    for i in 0..50 {
        events.push(ev!(
            &format!("c{i}"),
            1000 + i as i64 * 1000,
            "LlmApiCall",
            "connect",
            None,
            None,
            None,
            None,
            false,
        ));
    }
    assert!(evaluate(det0005, &events).is_some());
    assert!(evaluate(det0005, &events[..10]).is_none());
    Ok(())
}

#[test]
fn coverage_compiles_across_frameworks() -> anyhow::Result<()> {
    let rules = load_pack_dir(pack_dir())?;
    let coverage = build_coverage(&rules);
    assert_eq!(coverage.rule_count, 5);
    for framework in ["owasp_llm", "owasp_agentic", "attack"] {
        assert!(
            coverage
                .frameworks
                .get(framework)
                .map(|mapped| !mapped.is_empty())
                .unwrap_or(false),
            "framework {framework} has no coverage"
        );
    }
    Ok(())
}
