//! `fusebox breaker` — operator controls for the circuit breaker.
//!
//! Each subcommand talks to a running proxy over HTTP, so the binary
//! stays useful even when you're SSH'd into a server that hosts the
//! proxy in another process / container.

use anyhow::{anyhow, Context, Result};
use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Subcommand)]
pub enum BreakerCommand {
    /// Show the current breaker state for one tenant.
    Status(StatusArgs),
    /// List the breaker state for every tenant the proxy has seen.
    List(ListArgs),
    /// Manually flip a breaker back to Closed (operator override).
    Reset(ResetArgs),
}

#[derive(Debug, Args)]
pub struct StatusArgs {
    #[arg(long, env = "FUSEBOX_URL", default_value = "http://localhost:8080")]
    pub url: String,
    #[arg(long, default_value = "default")]
    pub tenant: String,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long, env = "FUSEBOX_URL", default_value = "http://localhost:8080")]
    pub url: String,
}

#[derive(Debug, Args)]
pub struct ResetArgs {
    #[arg(long, env = "FUSEBOX_URL", default_value = "http://localhost:8080")]
    pub url: String,
    #[arg(long)]
    pub tenant: String,
}

pub async fn run(cmd: BreakerCommand, _global_config: Option<PathBuf>) -> Result<()> {
    match cmd {
        BreakerCommand::Status(a) => status(a).await,
        BreakerCommand::List(a) => list(a).await,
        BreakerCommand::Reset(a) => reset(a).await,
    }
}

async fn status(args: StatusArgs) -> Result<()> {
    let url = format!("{}/v1/breaker/state", args.url.trim_end_matches('/'));
    let body: serde_json::Value = reqwest::Client::new()
        .get(&url)
        .header("x-fusebox-tenant", &args.tenant)
        .send()
        .await
        .with_context(|| format!("could not reach {url}"))?
        .error_for_status()
        .map_err(|e| anyhow!("status query failed: {e}"))?
        .json()
        .await
        .context("parse status response")?;
    let state_label = pretty_state(body.get("state"));
    println!("tenant   : {}", args.tenant);
    println!("breaker  : {state_label}");
    Ok(())
}

async fn list(args: ListArgs) -> Result<()> {
    let url = format!("{}/v1/breakers", args.url.trim_end_matches('/'));
    let body: serde_json::Value = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .with_context(|| format!("could not reach {url}"))?
        .error_for_status()
        .map_err(|e| anyhow!("list query failed: {e}"))?
        .json()
        .await
        .context("parse list response")?;

    let breakers = body
        .get("breakers")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if breakers.is_empty() {
        println!("(no breakers yet — proxy hasn't seen any traffic)");
        return Ok(());
    }
    println!("{:<24}  {}", "TENANT", "STATE");
    for b in breakers {
        let tenant = b
            .get("tenant")
            .and_then(|v| v.as_str())
            .unwrap_or("?")
            .to_string();
        let state = pretty_state(b.get("state"));
        println!("{:<24}  {}", tenant, state);
    }
    Ok(())
}

/// Stringify a `state` JSON node coming from one of two endpoint shapes:
/// `/v1/breaker/state` returns a plain label (`"closed"`); `/v1/breakers`
/// embeds the full `BreakerState` enum (`{"state":"open", ...}`).
fn pretty_state(node: Option<&serde_json::Value>) -> String {
    let Some(v) = node else { return "unknown".into() };
    if let Some(s) = v.as_str() {
        return s.to_string();
    }
    if let Some(s) = v.get("state").and_then(|x| x.as_str()) {
        return s.to_string();
    }
    // Fall back to compact JSON so the user at least sees something.
    serde_json::to_string(v).unwrap_or_else(|_| "?".into())
}

async fn reset(args: ResetArgs) -> Result<()> {
    let url = format!("{}/v1/breaker/reset", args.url.trim_end_matches('/'));
    let body: serde_json::Value = reqwest::Client::new()
        .post(&url)
        .json(&serde_json::json!({ "tenant": args.tenant }))
        .send()
        .await
        .with_context(|| format!("could not reach {url}"))?
        .error_for_status()
        .map_err(|e| anyhow!("reset failed: {e}"))?
        .json()
        .await
        .context("parse reset response")?;
    let from = body.get("from").and_then(|v| v.as_str()).unwrap_or("?");
    let to = body.get("to").and_then(|v| v.as_str()).unwrap_or("closed");
    println!("✔ {} breaker {from} → {to}", args.tenant);
    Ok(())
}
