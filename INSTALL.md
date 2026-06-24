# Pollen DEK Installation Guide

## Automated Installation

The easiest way to install Pollen DEK is using the provided scripts.

**Windows (PowerShell as Administrator):**

```powershell
.\scripts\windows_install.ps1
```

**macOS/Linux:**

```bash
sudo ./scripts/install.sh
```

## Post-Installation

Once installed, you can trigger the First-Run Wizard to configure the DEK:

```bash
pollen-dek wizard
```

Or you can accept the agreements from the command line:

```bash
pollen-dek agree
```

## Running the Service

Pollen DEK installs itself as a background service:

- On Linux, it uses `systemd` (`pollen-dek.service`).
- On Windows, it registers a Windows Service.
- On macOS, it registers a `launchd` daemon.

You can manage the service using:

```bash
pollen-dek service status
pollen-dek service stop
pollen-dek service start
```

## Diagnostics

To check your installation:

```bash
pollen-dek doctor
```

To export compliance evidence logs:

```bash
pollen-dek export-compliance
```
