// SPDX-License-Identifier: Apache-2.0
#![allow(clippy::print_stdout)]
// Copyright (c) 2026 AEC Infraconnect

use anyhow::{Context, Result};
use rcgen::{CertificateParams, DnType, IsCa};
use std::fs;
use std::path::Path;

fn main() -> Result<()> {
    let certs_dir = Path::new("../../certs");
    if !certs_dir.exists() {
        fs::create_dir_all(certs_dir).context("Failed to create certs directory")?;
    }

    // 1. Generate Root CA
    let mut root_params = CertificateParams::new(vec!["Pollen Cloud Root CA".to_string()])?;
    root_params.is_ca = IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    root_params
        .distinguished_name
        .push(DnType::OrganizationName, "Pollen DEK Project");
    root_params
        .distinguished_name
        .push(DnType::CommonName, "Pollen Cloud Root CA");
    let root_kp = rcgen::KeyPair::generate()?;
    let root_cert = root_params.self_signed(&root_kp)?;

    fs::write(certs_dir.join("root_ca.crt"), root_cert.pem())?;
    fs::write(certs_dir.join("root_ca.key"), root_kp.serialize_pem())?;
    println!("Root CA generated.");

    // 2. Generate Server Certificate (mock-cloud)
    let mut server_params =
        CertificateParams::new(vec!["localhost".to_string(), "127.0.0.1".to_string()])?;
    server_params
        .distinguished_name
        .push(DnType::OrganizationName, "Pollen DEK Project");
    server_params
        .distinguished_name
        .push(DnType::CommonName, "Pollen Mock Cloud Server");
    let server_kp = rcgen::KeyPair::generate()?;
    let server_cert = server_params.signed_by(&server_kp, &root_cert, &root_kp)?;

    fs::write(certs_dir.join("server.crt"), server_cert.pem())?;
    fs::write(certs_dir.join("server.key"), server_kp.serialize_pem())?;
    println!("Server Certificate generated.");

    // 3. Generate Client Certificate (DEK Telemetry / Sync)
    let mut client_params = CertificateParams::new(vec!["dek-client".to_string()])?;
    client_params
        .distinguished_name
        .push(DnType::OrganizationName, "Pollen DEK Project");
    client_params
        .distinguished_name
        .push(DnType::CommonName, "DEK Edge Client");
    let client_kp = rcgen::KeyPair::generate()?;
    let client_cert = client_params.signed_by(&client_kp, &root_cert, &root_kp)?;

    fs::write(certs_dir.join("client.crt"), client_cert.pem())?;
    fs::write(certs_dir.join("client.key"), client_kp.serialize_pem())?;
    println!("Client Certificate generated.");

    println!("All MTLS mock certificates successfully generated in `certs/`.");
    Ok(())
}
