use sha2::{Digest, Sha256};

pub fn sha256_string(input: &str) -> String {
    let mut h = Sha256::new();
    h.update(input.as_bytes());
    hex::encode(h.finalize())
}

pub fn redact_arg(arg: &str) -> String {
    let lower = arg.to_ascii_lowercase();
    if lower.contains("key")
        || lower.contains("token")
        || lower.contains("secret")
        || lower.starts_with("sk-")
        || lower.contains("authorization")
    {
        "<redacted>".to_string()
    } else if arg.len() > 180 {
        format!("{}…", &arg[..80])
    } else {
        arg.to_string()
    }
}

pub fn redact_path_for_ui(path: &str) -> String {
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_default();
    if !home.is_empty() {
        path.replace(&home, "${HOME}")
    } else {
        path.to_string()
    }
}

pub fn redact_command_line(cmd: &str) -> String {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let redacted_parts: Vec<String> = parts.into_iter().map(redact_arg).collect();
    redacted_parts.join(" ")
}
