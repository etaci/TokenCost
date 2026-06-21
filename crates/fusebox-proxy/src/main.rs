//! Binary entrypoint. Delegates to the library so `fusebox-cli start`
//! can boot the proxy in-process without forking.

use anyhow::Result;
use fusebox_proxy::{run, RunOptions};
use std::path::PathBuf;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let opts = RunOptions {
        config_path: std::env::var("FUSEBOX_CONFIG").ok().map(PathBuf::from),
        bind_override: std::env::var("FUSEBOX_BIND").ok(),
    };
    run(opts).await
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_target(false))
        .init();
}
