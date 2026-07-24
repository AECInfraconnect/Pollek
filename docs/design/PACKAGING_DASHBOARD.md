# Packaging the Local Admin Dashboard

The local admin dashboard is a static Vite/React bundle
(`apps/local-admin-dashboard`). At runtime it is **served by the
`local-control-plane` binary** (the Local Control Plane, LCP) on
`127.0.0.1:43891` via a `ServeDir` fallback with SPA rewrite to `index.html`
(`crates/local-control-plane/src/app.rs`).

This document defines **where the built dashboard assets live per OS**, how the
LCP finds them, and how the packaging pipeline puts them there.

## Standard install locations

The convention is: the dashboard ships in the platform's **shared-data**
location, next to the binaries, under a `pollek-dek/dashboard` subtree.

| OS | Binaries | Dashboard assets | Service that serves it |
|----|----------|------------------|------------------------|
| **Linux** (`.deb`) | `/usr/bin/` | `/usr/share/pollek-dek/dashboard/` | `pollek-dek-dashboard.service` (systemd) |
| **macOS** (`.pkg`) | `/usr/local/bin/` | `/usr/local/share/pollek-dek/dashboard/` | `ai.pollek.dek.dashboard` (launchd) |
| **Windows** (`.msi`) | `C:\Program Files\PollekDEK\` | `C:\Program Files\PollekDEK\dashboard\` | `PollekDEKDashboard` (Windows service) |

These follow each platform's norm: the FHS `/usr/share` (and `/usr/local/share`)
for arch-independent data on Unix, and a `dashboard\` folder beside the
executables on Windows.

## How the LCP resolves the dashboard directory

`crates/local-control-plane/src/config.rs :: resolve_dashboard_dir()` picks the
directory in this order (first match wins):

1. **`DEK_DASHBOARD_DIR`** — explicit operator override. Dev scripts and the
   service units set this; it always wins.
2. **Standard locations that actually contain `index.html`**, probed:
   - relative to the running executable first —
     `<exe_dir>/dashboard` (Windows/portable) and
     `<exe_dir>/../share/pollek-dek/dashboard` (Unix FHS), so a relocated
     install still resolves without configuration;
   - then the absolute per-OS system prefixes from the table above.
3. **`./apps/local-admin-dashboard/dist`** — in-repo dev fallback for
   `cargo run` from a checkout.

Because resolution is relative-to-binary first, the packaged services do **not**
strictly need `DEK_DASHBOARD_DIR`; the units set it anyway as an explicit
default. The selection logic is unit-tested in `config.rs`.

## How the packaging pipeline places the assets

`.github/workflows/package.yml` (runs on `v*.*.*` tags) builds the dashboard and
stages it into each installer:

- Every job runs `npm ci && npm run build` in `apps/local-admin-dashboard`, and
  builds `local-control-plane` alongside `dek-core` / `dek-mcp-proxy` /
  `dek-updater`.
- **Linux** — `[package.metadata.deb]` in `crates/dek-core/Cargo.toml` maps
  `apps/local-admin-dashboard/dist/**/*` → `/usr/share/pollek-dek/dashboard/`
  and installs the `local-control-plane` binary plus
  `pollek-dek-dashboard.service`.
- **Windows** — `heat.exe` harvests `dist/` into a WiX `ComponentGroup`
  (`HarvestedDashboard`, directory ref `DASHBOARDDIR`) referenced by
  `packaging/windows/pollek-dek.wxs`, which also installs
  `local-control-plane.exe` as the `PollekDEKDashboard` service.
- **macOS** — the assets are copied into
  `target/pkg/root/usr/local/share/pollek-dek/dashboard` and the
  `ai.pollek.dek.dashboard` LaunchDaemon is installed before `pkgbuild`.

### CI validation

`.github/workflows/release-dry-run.yml` runs on every PR into `main`. It builds
the dashboard, runs `cargo deb -p dek-core`, and asserts via `dpkg -c` that the
`.deb` contains `usr/share/pollek-dek/dashboard/index.html`,
`usr/share/pollek-dek/dashboard/assets/`, and `usr/bin/local-control-plane` —
so the Linux packaging path is covered on the PR. The Windows MSI and macOS PKG
paths are exercised by `package.yml` on a release tag.

## Auto-update

The signed release archives published by `.github/workflows/release.yml` and the
`dek-updater` crate handle updates: a new tag → CI publishes signed
`*.tar.gz` + `.sig` + `.pem` + `SHA256SUMS` to GitHub Releases → the installed
`dek-updater` checks the releases API, verifies cosign signature + checksum, and
swaps the binaries. Shipping the dashboard inside the same release archive keeps
the UI updating in lockstep with the binaries.
