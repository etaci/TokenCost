//! `fusebox start` — boot the proxy in-process.

use anyhow::Result;
use clap::Args;
use fusebox_proxy::{run as run_proxy, RunOptions};
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct StartArgs {
    /// Override the bind address from config (e.g. `0.0.0.0:9090`).
    #[arg(long, env = "FUSEBOX_BIND")]
    pub bind: Option<String>,
}

pub async fn run(args: StartArgs, config: Option<PathBuf>) -> Result<()> {
    let opts = RunOptions {
        config_path: config,
        bind_override: args.bind,
    };
    run_proxy(opts).await
}
