//! `fusebox status` — fetch breaker state from a running proxy.

use anyhow::{Context, Result};
use clap::Args;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct StatusArgs {
    /// Base URL of the proxy (e.g. http://localhost:8080).
    #[arg(long, env = "FUSEBOX_URL", default_value = "http://localhost:8080")]
    pub url: String,

    /// Tenant id to query (sent via X-Fusebox-Tenant).
    #[arg(long, default_value = "default")]
    pub tenant: String,
}

pub async fn run(args: StatusArgs, _config: Option<PathBuf>) -> Result<()> {
    let client = reqwest::Client::new();
    let url = format!("{}/v1/breaker/state", args.url.trim_end_matches('/'));
    let resp = client
        .get(&url)
        .header("x-fusebox-tenant", &args.tenant)
        .send()
        .await
        .with_context(|| format!("could not reach {url}"))?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.context("parse status response")?;
    if !status.is_success() {
        anyhow::bail!("status query failed: HTTP {status}: {body}");
    }

    let state = body
        .get("state")
        .and_then(|s| s.as_str())
        .unwrap_or("unknown");
    println!("tenant   : {}", args.tenant);
    println!("breaker  : {state}");
    Ok(())
}
