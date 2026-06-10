# Pollen DEK v1.0.0-beta.1 Release Notes

This release introduces end-to-end auto-update capabilities, OS installers, and kernel-level enforcement foundations.

## Highlights
- **Automated Installers**: `pollen-dek-linux-amd64.deb` and `pollen-dek-windows-amd64.zip` are now generated automatically.
- **End-to-End Auto-Updater**: `dek-updater` now performs atomic executable swaps with automatic rollback on health-check failure.
- **CLI Proxy**: `dek-cli update --channel beta` seamlessly invokes `dek-updater`.
- **Adaptive Policy Routing**: Automatic selection between Cedar, OpenFGA, OPA, and eBPF based on decision kind and complexity.
- **Kernel Guard**: eBPF network rules are now limited to 1024 exact match rules to prevent verifier limits/crashes, gracefully falling back to user-mode PDP.

## Downloads and Verification

| OS | File |
|---|---|
| Linux (deb) | `pollen-dek-linux-amd64.deb` |
| Linux (tar) | `pollen-dek-linux-amd64.tar.gz` |
| Windows (zip) | `pollen-dek-windows-amd64.zip` |
| macOS (tar) | `pollen-dek-macos-amd64.tar.gz` |

To verify the integrity of these artifacts, download `SHA256SUMS` and `SHA256SUMS.sig`, then run:
```bash
sha256sum -c SHA256SUMS

cosign verify-blob \
  --certificate pollen-dek-linux-amd64.tar.gz.pem \
  --signature pollen-dek-linux-amd64.tar.gz.sig \
  --certificate-identity-regexp "^https://github.com/AECInfraconnect/AntiG_Pollen_DEK/.*" \
  --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
  pollen-dek-linux-amd64.tar.gz
```
