mod ffi;

use anyhow::Result;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    info!("starting Pollen DEK WFP service");

    ffi::init_provider()?;

    // In real implementation, load compiled policy from DEK Local PDP Host.
    ffi::add_tcp_block_443()?;

    info!("WFP policy installed");

    // Windows service loop placeholder.
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    }
}
