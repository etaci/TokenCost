//! Subcommand parsing + dispatch.

mod breaker;
mod budget;
mod config_cmd;
mod doctor;
mod pricing;
mod start;
mod status;
mod tail;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "fusebox",
    version,
    about = "Cost circuit breaker for AI agents",
    long_about = "Fusebox protects you from runaway LLM bills. Run `fusebox start` to boot the proxy, then point your OpenAI / Anthropic clients at http://localhost:8080."
)]
pub struct Cli {
    /// Path to a fusebox.yaml. Defaults to `./fusebox.yaml` if it exists,
    /// otherwise compiled-in defaults.
    #[arg(long, env = "FUSEBOX_CONFIG", global = true)]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Start the proxy in the foreground.
    Start(start::StartArgs),

    /// Show breaker / spend status across tenants.
    Status(status::StatusArgs),

    /// Live-tail spend events (Ctrl-C to stop).
    Tail(tail::TailArgs),

    /// Manage local fusebox.yaml.
    #[command(subcommand)]
    Config(config_cmd::ConfigCommand),

    /// Read / write per-tenant budgets.
    #[command(subcommand)]
    Budget(budget::BudgetCommand),

    /// Inspect or override the circuit breaker.
    #[command(subcommand)]
    Breaker(breaker::BreakerCommand),

    /// Manage the pricing tables embedded by the proxy.
    #[command(subcommand)]
    Pricing(pricing::PricingCommand),

    /// Sanity-check ledger / pricing / config.
    Doctor(doctor::DoctorArgs),
}

impl Cli {
    pub async fn run(self) -> Result<()> {
        match self.command {
            Command::Start(args) => start::run(args, self.config).await,
            Command::Status(args) => status::run(args, self.config).await,
            Command::Tail(args) => tail::run(args, self.config).await,
            Command::Config(cmd) => config_cmd::run(cmd, self.config).await,
            Command::Budget(cmd) => budget::run(cmd, self.config).await,
            Command::Breaker(cmd) => breaker::run(cmd, self.config).await,
            Command::Pricing(cmd) => pricing::run(cmd).await,
            Command::Doctor(args) => doctor::run(args, self.config).await,
        }
    }
}
