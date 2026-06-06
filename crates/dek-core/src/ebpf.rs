use tracing::{info, warn};

#[cfg(target_os = "linux")]
pub fn probe_ebpf_support() -> bool {
    // Basic checks for eBPF support on Linux:
    // 1. Check if running as root
    // 2. Check for BTF support
    let is_root = unsafe { libc::geteuid() == 0 };
    let has_btf = std::path::Path::new("/sys/kernel/btf/vmlinux").exists();
    
    if !is_root {
        warn!("eBPF requires root privileges. Falling back to App-Layer-Only.");
    }
    if !has_btf {
        warn!("Kernel BTF (/sys/kernel/btf/vmlinux) not found. Falling back to App-Layer-Only.");
    }
    
    is_root && has_btf
}

#[cfg(target_os = "linux")]
pub fn load_and_attach() -> anyhow::Result<()> {
    info!("Probing eBPF support...");
    if !probe_ebpf_support() {
        warn!("eBPF support check failed. Gracefully degrading to App-Layer-Only (Layer 3/7).");
        return Ok(());
    }

    info!("Initializing WS-D eBPFD Subsystem...");
    
    // eBPFD is spawned asynchronously to manage BPF maps and ringbuf
    // Passing the cgroup path of the supervised processes
    let cgroup_path = "/sys/fs/cgroup/pollen-dek-supervised";
    
    // Start supervisor logic
    tokio::spawn(async move {
        if let Err(e) = dek_ebpfd::daemon::start_ebpfd_supervisor(cgroup_path).await {
            tracing::error!("eBPFD Supervisor failed: {}", e);
        }
    });

    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn load_and_attach() -> anyhow::Result<()> {
    info!("Layer 2 eBPF WS-D guardrails are skipped on non-Linux platforms.");
    warn!("Platform relies solely on App-layer MCP and opt-in proxy redirect.");
    Ok(())
}
