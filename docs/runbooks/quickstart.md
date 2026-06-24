# Pollen DEK Quickstart

This guide will walk you through the process of setting up and running Pollen DEK.

## Prerequisites

- Windows, macOS, or Linux.
- Administrator / root privileges for installation.
- (Optional) Docker for running local mock dependencies.

## Installation

Run the installer script:
For Windows (PowerShell as Administrator):

```powershell
.\scripts\windows_install.ps1
```

For Linux/macOS:

```bash
sudo ./scripts/install.sh
```

## First-Run Experience

After installation, the First-Run Wizard will be launched automatically. Or you can trigger it manually:

```bash
pollen-dek wizard
```

This will launch a local web UI where you can:

1. Accept the EULA and Privacy Notice.
2. Select your Deployment Profile (Sovereign Mode vs Cloud-Managed).
3. Authorize the first scan of local AI agents.

Alternatively, you can accept the agreements via the CLI:

```bash
pollen-dek agree
```

## Sovereign Mode (Air-Gap Ready)

Pollen DEK can run completely locally with no outbound internet connection. To enable this:

```bash
pollen-dek profile set local
```

This will configure the DEK to not send telemetry to the cloud and block egress data exfiltration.

## Verify Installation

To check if the DEK is healthy and running:

```bash
pollen-dek status
pollen-dek health
```

To run diagnostic checks on your environment:

```bash
pollen-dek doctor
```

## Exporting Compliance Evidence

If you are undergoing an audit (e.g. EU AI Act, NIST AI RMF, ISO 42001), you can export a compliance evidence pack containing tamper-evident audit logs:

```bash
pollen-dek export-compliance
```
