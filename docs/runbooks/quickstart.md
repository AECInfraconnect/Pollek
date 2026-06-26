# Pollek DEK Quickstart

This guide will walk you through the process of setting up and running Pollek DEK.

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
pollek-dek wizard
```

This will launch a local web UI where you can:

1. Accept the EULA and Privacy Notice.
2. Select your operating mode: Simple, Advance, or Enterprise Cloud after Pollek Cloud connects successfully.
3. Authorize the first scan of local AI agents.

Alternatively, you can accept the agreements via the CLI:

```bash
pollek-dek agree
```

## Local-Only Operation (Air-Gap Ready)

Pollek DEK can run completely locally with no outbound internet connection. To enable this:

```bash
pollek-dek profile set local
```

This will configure the DEK to not send telemetry to the cloud and block egress data exfiltration.

## Verify Installation

To check if the DEK is healthy and running:

```bash
pollek-dek status
pollek-dek health
```

To run diagnostic checks on your environment:

```bash
pollek-dek doctor
```

## Exporting Compliance Evidence

If you are undergoing an audit (e.g. EU AI Act, NIST AI RMF, ISO 42001), you can export a compliance evidence pack containing tamper-evident audit logs:

```bash
pollek-dek export-compliance
```
