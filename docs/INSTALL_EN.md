# Pollek Local Enforcement Kit Installation Guide (v1.0.0-beta)

## System Requirements

- OS: Windows 10/11, macOS 12+, or Ubuntu 20.04+
- Storage: 100MB free space
- Privileges: Administrator/root access required

> **Note on Simple Mode**: If you plan to use Pollek in **Simple Mode**, you do **not** need to configure any complex PEPs (like eBPF or WFP). The system automatically manages enforcement capabilities transparently based on your OS and privileges.

## Preflight Check
Run the doctor tool before installation to ensure your system is fully compatible:
```bash
pollek-dekctl doctor
```

## Windows Installation

1. Download `Pollek-dek-x86_64-pc-windows-msvc.msi` from the GitHub Releases page.
2. Double-click the MSI installer and follow the prompts.
3. The `PollenDEKCore` service will be installed and started automatically in the background.

## Linux Installation

1. Download the `.deb` release matching your architecture (e.g., `Pollek-dek-x86_64-unknown-linux-gnu.deb` or `aarch64`).
2. Install via dpkg: `sudo dpkg -i Pollek-dek-*.deb`
3. The `Pollek-Local Enforcement Kit.service` systemd service will be automatically enabled and started.

## macOS Installation

1. Download the `.pkg` release (e.g., `Pollek-dek-x86_64-apple-darwin.pkg`).
2. Run the installer package.
3. The `ai.Pollek.Local Enforcement Kit` launchd agent will load automatically.

## Verification

Run `Pollek-dekctl status` to verify installation and service health.
