use anyhow::Result;
use axum::{
    extract::{Request, State},
    http::{header::AUTHORIZATION, StatusCode},
    middleware::Next,
    response::Response,
};
use rand::RngCore;
use std::fs;
use std::path::Path;

pub fn load_or_create_token(data_dir: &Path) -> Result<String> {
    let token_path = data_dir.join("api_token");
    if token_path.exists() {
        let token = fs::read_to_string(&token_path)?.trim().to_string();
        Ok(token)
    } else {
        let mut buf = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut buf);
        let token = hex::encode(buf);

        if let Some(parent) = token_path.parent() {
            fs::create_dir_all(parent).ok();
        }

        fs::write(&token_path, &token)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&token_path, fs::Permissions::from_mode(0o600));
        }

        Ok(token)
    }
}

/// Constant time string comparison
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0;
    for (x, y) in a.bytes().zip(b.bytes()) {
        result |= x ^ y;
    }
    result == 0
}

pub async fn require_token(
    State(state): State<crate::state::AppState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if state.auth_disabled {
        return Ok(next.run(req).await);
    }

    let auth_header = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok());
    let provided_token = match auth_header {
        Some(h) if h.starts_with("Bearer ") => h.trim_start_matches("Bearer ").trim(),
        _ => return Err(StatusCode::UNAUTHORIZED),
    };

    if !constant_time_eq(provided_token, &state.api_token) {
        tracing::warn!("Unauthorized access attempt with invalid token");
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(req).await)
}
