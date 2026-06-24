# Pollen DEK: 5-Minute Quickstart

## 1. Install DEK Service

Run the installer for your OS:

- **Windows**: `pwsh -c "irm https://pollen.aecinfraconnect.com/install.ps1 | iex"`
- **macOS / Linux**: `curl -sSL https://pollen.aecinfraconnect.com/install.sh | bash`

## 2. Launch Wizard & Consent

Run `pollen-dek wizard` to open the First-Run Wizard in your browser. Read and accept the EULA and Privacy Policy.

## 3. Verify Health

Run `pollen-dek doctor` to ensure your system meets all runtime dependencies (e.g. `WinDivert` on Windows, or `eBPF` on Linux).

## 4. Choose Profile

Run `pollen-dek profile set local` to operate in standalone mode, or `pollen-dek profile set cloud --url <tenant_url>` to connect to Pollen Cloud.
To enable fully air-gapped Sovereign Mode: `pollen-dek profile set sovereign`.

## 5. View Policies

Run `pollen-dek status` to see the currently enforced policies and routing status.

## 6. Export Compliance (Optional)

Run `pollen-dek export-compliance` to generate a markdown report for your compliance officers (maps to EU AI Act, NIST AI RMF, ISO 42001).
