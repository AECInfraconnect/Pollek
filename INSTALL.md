# Pollek DEK Installation Guide

## Automated Installation

The easiest way to install Pollek DEK is using the provided scripts.

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
pollek-dek wizard
```

Or you can accept the agreements from the command line:

```bash
pollek-dek agree
```

## Running the Service

Pollek DEK installs itself as a background service:

- On Linux, it uses `systemd` (`pollek-dek.service`).
- On Windows, it registers a Windows Service.
- On macOS, it registers a `launchd` daemon.

You can manage the service using:

```bash
pollek-dek service status
pollek-dek service stop
pollek-dek service start
```

## Diagnostics

To check your installation:

```bash
pollek-dek doctor
```

To export compliance evidence logs:

```bash
pollek-dek export-compliance
```
