use std::path::PathBuf;

pub fn get_config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("DEK_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    
    #[cfg(target_os = "linux")]
    {
        PathBuf::from("/etc/pollen-dek")
    }
    #[cfg(target_os = "windows")]
    {
        let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
        PathBuf::from(program_data).join("PollenDEK").join("config")
    }
    #[cfg(target_os = "macos")]
    {
        PathBuf::from("/Library/Application Support/PollenDEK/config")
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        PathBuf::from(".")
    }
}

pub fn get_data_dir() -> PathBuf {
    // This represents the State directory
    if let Ok(dir) = std::env::var("DEK_STATE_DIR") {
        return PathBuf::from(dir);
    }
    if let Ok(dir) = std::env::var("DEK_DATA_DIR") {
        return PathBuf::from(dir);
    }

    #[cfg(target_os = "linux")]
    {
        PathBuf::from("/var/lib/pollen-dek")
    }
    #[cfg(target_os = "windows")]
    {
        let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
        PathBuf::from(program_data).join("PollenDEK").join("state")
    }
    #[cfg(target_os = "macos")]
    {
        PathBuf::from("/var/db/pollen-dek")
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        PathBuf::from(".")
    }
}

pub fn get_log_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("DEK_LOG_DIR") {
        return PathBuf::from(dir);
    }

    #[cfg(target_os = "linux")]
    {
        PathBuf::from("/var/log/pollen-dek")
    }
    #[cfg(target_os = "windows")]
    {
        let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
        PathBuf::from(program_data).join("PollenDEK").join("logs")
    }
    #[cfg(target_os = "macos")]
    {
        PathBuf::from("/Library/Logs/PollenDEK")
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        PathBuf::from("logs")
    }
}

pub fn get_runtime_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("DEK_RUNTIME_DIR") {
        return PathBuf::from(dir);
    }

    #[cfg(target_os = "linux")]
    {
        PathBuf::from("/run/pollen-dek")
    }
    #[cfg(target_os = "windows")]
    {
        // On Windows, runtime paths for named pipes are usually \\.\pipe\...
        // But for consistent path usage, we might just use a runtime directory.
        let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
        PathBuf::from(program_data).join("PollenDEK").join("run")
    }
    #[cfg(target_os = "macos")]
    {
        PathBuf::from("/var/run/pollen-dek")
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        PathBuf::from("run")
    }
}

pub fn get_plugin_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("DEK_PLUGIN_DIR") {
        return PathBuf::from(dir);
    }

    get_data_dir().join("plugins")
}

pub fn get_bootstrap_path() -> PathBuf {
    if let Ok(file) = std::env::var("DEK_BOOTSTRAP_PATH") {
        return PathBuf::from(file);
    }
    get_config_dir().join("bootstrap.json")
}

pub fn get_active_bundle_path() -> PathBuf {
    if let Ok(file) = std::env::var("DEK_BUNDLE_PATH") {
        return PathBuf::from(file);
    }
    get_data_dir().join("active_bundle.json")
}
