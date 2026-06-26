// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use anyhow::{Context, Result};
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub prerelease: bool,
    pub assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
pub struct GitHubAsset {
    pub name: String,
    pub browser_download_url: String,
}

const REPO_API: &str = "https://api.github.com/repos/AECInfraconnect/Pollek/releases";
const OIDC_ISSUER: &str = "https://token.actions.githubusercontent.com";
const IDENTITY_REGEXP: &str = "^https://github.com/AECInfraconnect/Pollek/.*";

pub fn latest_release(channel: &str) -> Result<GitHubRelease> {
    let resp: Vec<GitHubRelease> = crate::http_client()
        .get(REPO_API)
        .set("User-Agent", "pollek-dek-updater")
        .call()
        .context("Failed to fetch releases from GitHub")?
        .into_json()?;

    for release in resp {
        if channel == "stable" && release.prerelease {
            continue;
        }
        return Ok(release);
    }

    anyhow::bail!("No releases found for channel {}", channel)
}

pub fn is_newer(current_version: &str, latest_tag: &str) -> Result<bool> {
    // Strip leading 'v' if present
    let current = current_version.trim_start_matches('v');
    let latest = latest_tag.trim_start_matches('v');

    let current_ver = Version::parse(current).unwrap_or(Version::new(0, 0, 0));
    let latest_ver = match Version::parse(latest) {
        Ok(v) => v,
        Err(_) => {
            eprintln!(
                "Failed to parse remote version {}; acting conservative",
                latest
            );
            return Ok(false);
        }
    };

    Ok(latest_ver > current_ver)
}

pub fn download_update(
    release: &GitHubRelease,
    temp_dir: &Path,
) -> Result<(PathBuf, PathBuf, PathBuf, PathBuf)> {
    let platform = if cfg!(windows) {
        "windows-amd64.exe.tar.gz"
    } else if cfg!(target_os = "macos") {
        "macos-amd64.tar.gz"
    } else {
        "linux-amd64.tar.gz"
    };

    let mut archive_url = None;
    let mut sig_url = None;
    let mut pem_url = None;
    let mut sum_url = None;

    for asset in &release.assets {
        if asset.name.contains(platform)
            && !asset.name.ends_with(".sig")
            && !asset.name.ends_with(".pem")
        {
            archive_url = Some(&asset.browser_download_url);
        } else if asset.name.contains(platform) && asset.name.ends_with(".sig") {
            sig_url = Some(&asset.browser_download_url);
        } else if asset.name.contains(platform) && asset.name.ends_with(".pem") {
            pem_url = Some(&asset.browser_download_url);
        } else if asset.name == "SHA256SUMS" {
            sum_url = Some(&asset.browser_download_url);
        }
    }

    let archive_url = archive_url.context(format!(
        "Could not find archive asset for platform {}",
        platform
    ))?;
    let sig_url = sig_url.context("Could not find .sig asset")?;
    let pem_url = pem_url.context("Could not find .pem asset")?;
    let sum_url = sum_url.context("Could not find SHA256SUMS asset")?;

    let archive_path = download_file(archive_url, temp_dir, "archive.tar.gz")?;
    let sig_path = download_file(sig_url, temp_dir, "archive.tar.gz.sig")?;
    let pem_path = download_file(pem_url, temp_dir, "archive.tar.gz.pem")?;
    let sum_path = download_file(sum_url, temp_dir, "SHA256SUMS")?;

    Ok((archive_path, sum_path, sig_path, pem_path))
}

fn download_file(url: &str, dir: &Path, filename: &str) -> Result<PathBuf> {
    let resp = crate::http_client()
        .get(url)
        .call()
        .context(format!("Failed to download {}", url))?;
    let mut reader = resp.into_reader();

    let path = dir.join(filename);
    let mut file = fs::File::create(&path)?;
    std::io::copy(&mut reader, &mut file)?;

    Ok(path)
}

pub fn verify_sha256(archive_path: &Path, sum_path: &Path) -> Result<()> {
    let filename = archive_path
        .file_name()
        .context("Failed to get file name")?
        .to_string_lossy();
    let sums = fs::read_to_string(sum_path)?;

    let mut expected_hash = None;
    for line in sums.lines() {
        if line.contains(filename.as_ref()) {
            expected_hash = Some(
                line.split_whitespace()
                    .next()
                    .context("Missing hash in line")?
                    .to_string(),
            );
            break;
        }
    }

    // In our CI, the asset is usually named pollek-dek-linux-amd64.tar.gz, but we saved it as archive.tar.gz
    // Let's just find the first hash in the sums file if we couldn't match the filename exactly,
    // assuming the sums file has the correct asset hashes.
    let expected_hash = match expected_hash {
        Some(h) => h,
        None => {
            // Find the hash that corresponds to our platform
            let platform_str = if cfg!(windows) {
                "windows-amd64"
            } else if cfg!(target_os = "macos") {
                "macos-amd64"
            } else {
                "linux-amd64"
            };
            let mut found = None;
            for line in sums.lines() {
                if line.contains(platform_str) {
                    found = Some(
                        line.split_whitespace()
                            .next()
                            .context("Missing hash in line")?
                            .to_string(),
                    );
                    break;
                }
            }
            found.context("Could not find matching hash in SHA256SUMS")?
        }
    };

    let mut file = fs::File::open(archive_path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 8192];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    let actual_hash = hex::encode(hasher.finalize());
    if actual_hash != expected_hash {
        anyhow::bail!(
            "SHA256 mismatch! Expected {}, got {}",
            expected_hash,
            actual_hash
        );
    }

    Ok(())
}

pub fn verify_cosign(archive_path: &Path, sig_path: &Path, pem_path: &Path) -> Result<()> {
    // Requires cosign CLI to be installed and available in PATH
    let output = Command::new("cosign")
        .arg("verify-blob")
        .arg("--certificate")
        .arg(pem_path)
        .arg("--signature")
        .arg(sig_path)
        .arg("--certificate-identity-regexp")
        .arg(IDENTITY_REGEXP)
        .arg("--certificate-oidc-issuer")
        .arg(OIDC_ISSUER)
        .arg(archive_path)
        .output()
        .context("Failed to execute cosign CLI. Is it installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Cosign verification failed: {}", stderr);
    }

    Ok(())
}

pub fn verify_all(
    archive_path: &Path,
    sum_path: &Path,
    sig_path: &Path,
    pem_path: &Path,
) -> Result<()> {
    verify_sha256(archive_path, sum_path)?;
    verify_cosign(archive_path, sig_path, pem_path)?;
    Ok(())
}
