# Pollek DEK: 5-Minute Quickstart

## 1. Install DEK Service

Run the installer for your OS:

- **Windows**: `pwsh -c "irm https://pollek.aecinfraconnect.com/install.ps1 | iex"`
- **macOS / Linux**: `curl -sSL https://pollek.aecinfraconnect.com/install.sh | bash`

## 2. Launch Wizard & Consent

Run `pollek-dek wizard` to open the First-Run Wizard in your browser. Read and accept the EULA and Privacy Policy.

## 3. Verify Health

Run `pollek-dek doctor` to ensure your system meets all runtime dependencies (e.g. `WinDivert` on Windows, or `eBPF` on Linux).

## 4. Choose Profile

Run `pollek-dek profile set local` to operate in standalone local mode, or `pollek-dek profile set cloud --url <tenant_url>` to configure Pollek Cloud. Enterprise Cloud mode becomes available only after the cloud connection probe succeeds.

## 5. View Policies

Run `pollek-dek status` to see the currently enforced policies and routing status.

## 6. Export Compliance (Optional)

Run `pollek-dek export-compliance` to generate a markdown report for your compliance officers (maps to EU AI Act, NIST AI RMF, ISO 42001).
