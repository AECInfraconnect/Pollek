use anyhow::Result;
use serde::{Deserialize, Serialize};
use sysinfo::{ProcessesToUpdate, System};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessEvidence {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub process_name: String,
    pub exe_path_hash: Option<String>,
    pub exe_path_redacted: Option<String>,
    pub cmd_template: Vec<String>,
    pub cwd_hash: Option<String>,
    pub started_at_unix: Option<u64>,
}

pub fn scan_processes() -> Result<Vec<ProcessEvidence>> {
    let mut sys = System::new_all();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let mut out = Vec::new();
    for (pid, p) in sys.processes() {
        let exe = p.exe().map(|x| x.to_string_lossy().to_string());
        let cwd = p.cwd().map(|x| x.to_string_lossy().to_string());
        let cmd_template = p.cmd().iter().map(|s| crate::redaction::redact_arg(&s.to_string_lossy())).collect();

        out.push(ProcessEvidence {
            pid: pid.as_u32(),
            parent_pid: p.parent().map(|x| x.as_u32()),
            process_name: p.name().to_string_lossy().to_string(),
            exe_path_hash: exe.as_ref().map(|s| crate::redaction::sha256_string(s)),
            exe_path_redacted: exe.map(|s| crate::redaction::redact_path_for_ui(&s)),
            cmd_template,
            cwd_hash: cwd.as_ref().map(|s| crate::redaction::sha256_string(s)),
            started_at_unix: Some(p.start_time()),
        });
    }
    Ok(out)
}
