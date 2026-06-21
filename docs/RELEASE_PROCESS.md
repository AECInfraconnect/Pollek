# Pollen DEK Release Process

This document defines the strict, required process for releasing a new version of Pollen DEK, ensuring that re-tagging failures, dirty histories, and incomplete CI artifacts do not block releases.

## 1. Pre-flight Checklist

Before any tag is pushed to GitHub, you **MUST** verify the following:

- [ ] **Working Tree is Clean**: Ensure there are absolutely no uncommitted files or unstaged changes.
- [ ] **Lockfiles are Synchronized**: Ensure `cargo update` has been run and `Cargo.lock` is fully synced.
- [ ] **Integration Tests Pass Locally**: Do not push if `cargo test --locked` fails. Ensure `junit.xml` generation works and that no internal test builds block execution (`DEK_SKIP_HARNESS_BUILD=1` must be respected).
- [ ] **Formatting and Lints Pass**: Run `cargo clippy --workspace --locked --all-targets -- -D warnings` and `cargo fmt --all -- --check`.
- [ ] **CI is Green on `main`**: Check GitHub Actions. Do **NOT** create a tag on a commit that hasn't fully passed the CI pipeline on the `main` branch.

## 2. Correct Tagging Commands

Do not tag blindly. Tags should only be applied to a green commit on `main`.

1. **Pull the latest green commit**:

   ```bash
   git checkout main
   git pull origin main
   ```

2. **Create the tag**:

   ```bash
   git tag v1.0.0-beta.5
   ```

3. **Push ONLY the single tag**:

   ```bash
   git push origin v1.0.0-beta.5
   ```

   *(Do not use `git push --tags` as it pushes all local tags, potentially triggering multiple pipeline runs for outdated/abandoned tags).*

## 3. Tag Recovery (Fixing a Botched Release)

If a tag was created on a failing commit or if the release pipeline fails halfway, follow this procedure strictly. Do not just keep adding new tags to test CI.

1. **Delete the tag locally**:

   ```bash
   git tag -d v1.0.0-beta.5
   ```

2. **Delete the tag remotely**:

   ```bash
   git push origin :refs/tags/v1.0.0-beta.5
   ```

3. **Fix the underlying code**, push to `main`, wait for the `main` pipeline to turn **green**, and then restart the tagging process.

## 4. Beta vs GA Artifact Blocking Matrix

Not all artifacts are required for every release tier. The CI pipeline will enforce these checks.

| Artifact / Requirement | Beta Release | General Availability (GA) | Notes |
| :--- | :--- | :--- | :--- |
| **All Unit/Integration Tests Pass** | 🟢 Required | 🟢 Required | Includes JUnit generation. |
| **Gitleaks Audit** | 🟢 Required | 🟢 Required | Hard block. Must have 0 leaked keys. |
| **Code Signing (Authenticode/Apple)** | 🟡 Optional | 🟢 Required | Beta builds may distribute unsigned binaries. |
| **eBPF Hardening Checks** | 🟡 Optional | 🟢 Required | Fallback user-mode proxies are acceptable in Beta. |
| **Documentation Sync** | 🟢 Required | 🟢 Required | README and ARCHITECTURE must be up to date. |
| **Performance/Soak Tests** | ⚪ Skipped | 🟢 Required | Soak tests run manually or on a schedule for GA. |
