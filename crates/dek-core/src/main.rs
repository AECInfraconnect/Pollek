// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

#![warn(clippy::print_stdout, clippy::print_stderr)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod api;

mod ebpf;
mod ipc_client;
mod ipc_server;
mod kernel_guard;
mod keystore_migration;
pub mod capabilities;

mod network_loop;
mod probation;
mod reload_coordinator;
mod service_integration;
mod supervisor;
mod svid_renewal;
mod svid_renewal_failclosed;

fn main() -> anyhow::Result<()> {
    #[cfg(windows)]
    service_integration::run_as_service_if_needed(async { run().await })?;
    #[cfg(not(windows))]
    service_integration::run_as_service_if_needed(async { run().await })?;
    Ok(())
}

async fn run() -> anyhow::Result<()> {
    supervisor::Supervisor::bootstrap().await?.run().await
}
