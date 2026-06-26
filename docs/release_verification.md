# Release Verification Guide

This guide explains how to verify the authenticity and integrity of Pollek Local Enforcement Kit `v1.0.0-beta` release artifacts.

## 1. Checksums Verification

Each release includes a `SHA256SUMS` file. You can verify that your downloaded binary matches the checksum using the following commands:

**Linux / macOS:**

```bash
sha256sum --check SHA256SUMS
# Alternatively, if downloading a specific file:
sha256sum Pollek-dek-linux-x64.tar.gz
# Compare the output with the hash in SHA256SUMS
```

**Windows (PowerShell):**

```powershell
Get-FileHash .\Pollek-dek-windows-x64.zip -Algorithm SHA256
```

## 2. Sigstore Cosign Verification (Keyless Signature)

The release artifacts are signed using Sigstore Cosign via keyless OIDC signatures bound to the GitHub Actions workflow.

To verify the signature of a release artifact:

1. Install `cosign` (<https://github.com/sigstore/cosign#installation>)
2. Run the verification command against the downloaded artifact, its signature, and its certificate:

```bash
cosign verify-blob \
  --certificate Pollek-dek-linux-x64.tar.gz.pem \
  --signature Pollek-dek-linux-x64.tar.gz.sig \
  --certificate-identity "https://github.com/AECInfraconnect/Pollek/.github/workflows/release-gate.yml@refs/tags/v1.0.0-beta.1" \
  --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
  Pollek-dek-linux-x64.tar.gz
```

_Note: Replace `v1.0.0-beta.1` with the exact tag of the release you downloaded._

## 3. SBOM (Software Bill of Materials)

A CycloneDX SBOM is included with every release. It contains the full dependency tree of the Pollek Local Enforcement Kit application, ensuring supply-chain transparency.

You can inspect the SBOM using tools like `syft` or upload it to your vulnerability management system.
