use crate::paths::get_log_dir;
use std::fs;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn init_logging(service_name: &str) -> anyhow::Result<()> {
    #[cfg(target_os = "linux")]
    {
        // On Linux, log to stderr and let journald handle rotation.
        tracing_subscriber::registry()
            .with(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
            .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
            .init();
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    {
        // On Windows/macOS, use tracing-appender for daily rolling files.
        let log_dir = get_log_dir();
        fs::create_dir_all(&log_dir)?;

        // Ensure proper permissions where applicable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&log_dir, fs::Permissions::from_mode(0o750)).ok();
        }

        #[cfg(windows)]
        {
            // Set Windows ACLs to SYSTEM and Administrators only
            // For simplicity, we assume the directory is created by a privileged user/service
            // and we rely on parent inheritance or manual setups in this snippet unless we use winapi.
            // A robust implementation would use `winapi` or `windows-sys` to explicitly SetNamedSecurityInfo.
        }

        let file_appender = tracing_appender::rolling::daily(&log_dir, format!("{}.log", service_name));
        let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
        
        // Save the guard globally so it doesn't drop. Usually this is returned, but for simplicity
        // we can leak it or store it globally. Leaking is acceptable for the lifetime of the daemon.
        Box::leak(Box::new(_guard));

        tracing_subscriber::registry()
            .with(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
            .with(tracing_subscriber::fmt::layer().with_writer(std::io::stdout)) // stdout
            .with(tracing_subscriber::fmt::layer().with_writer(non_blocking)) // file
            .init();

        // Spawn a background task to sweep old logs
        spawn_log_sweeper(log_dir);
        
        Ok(())
    }
}

#[cfg(not(target_os = "linux"))]
fn spawn_log_sweeper(log_dir: std::path::PathBuf) {
    tokio::spawn(async move {
        let retention_days = 7;
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600 * 24)); // check daily
        loop {
            interval.tick().await;
            if let Ok(entries) = std::fs::read_dir(&log_dir) {
                let now = std::time::SystemTime::now();
                for entry in entries.flatten() {
                    if let Ok(meta) = entry.metadata() {
                        if let Ok(modified) = meta.modified() {
                            if let Ok(duration) = now.duration_since(modified) {
                                if duration.as_secs() > retention_days * 24 * 3600 {
                                    let _ = std::fs::remove_file(entry.path());
                                    tracing::info!("Swept old log file: {:?}", entry.path());
                                }
                            }
                        }
                    }
                }
            }
        }
    });
}
