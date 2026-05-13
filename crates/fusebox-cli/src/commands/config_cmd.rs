//! `fusebox config` — local config file management.

use anyhow::{anyhow, Result};
use clap::Subcommand;
use fusebox_core::Config;
use std::path::{Path, PathBuf};

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Write a starter `fusebox.yaml` into the current directory.
    Init {
        /// Where to write the file.
        #[arg(long, default_value = "fusebox.yaml")]
        path: PathBuf,
        /// Overwrite the file even if it already exists.
        #[arg(long)]
        force: bool,
    },
    /// Parse a config file and report what would actually take effect.
    Validate {
        /// Path to the config file (defaults to ./fusebox.yaml).
        path: Option<PathBuf>,
    },
}

pub async fn run(cmd: ConfigCommand, _global_config: Option<PathBuf>) -> Result<()> {
    match cmd {
        ConfigCommand::Init { path, force } => init(&path, force),
        ConfigCommand::Validate { path } => validate(path.as_deref()),
    }
}

fn init(path: &Path, force: bool) -> Result<()> {
    if path.exists() && !force {
        return Err(anyhow!(
            "{} already exists; pass --force to overwrite",
            path.display()
        ));
    }
    std::fs::write(path, STARTER)?;
    println!("wrote {}", path.display());
    Ok(())
}

fn validate(path: Option<&Path>) -> Result<()> {
    let cwd = PathBuf::from("fusebox.yaml");
    let actual = path.unwrap_or(&cwd);
    if !actual.exists() {
        return Err(anyhow!("{} does not exist", actual.display()));
    }
    let cfg = Config::from_yaml_file(actual).map_err(|e| anyhow!(e.to_string()))?;
    println!("✔ {} parsed", actual.display());
    println!("  bind                    : {}", cfg.proxy.bind);
    println!("  upstream_timeout_secs   : {}", cfg.proxy.upstream_timeout_secs);
    println!("  providers               : {} entries", cfg.providers.len());
    println!(
        "  default_budget          : ${} per {}",
        cfg.policy.default_budget.limit_usd,
        cfg.policy.default_budget.window.as_label()
    );
    Ok(())
}

const STARTER: &str = r#"# Fusebox starter config — adjust to taste, then run `fusebox start`.

proxy:
  bind: 0.0.0.0:8080
  upstream_timeout_secs: 600

storage:
  type: sqlite
  path: .fusebox/data.db

policy:
  default_budget:
    limit_usd: 50.0
    window: day
    label: default
  breaker_cooldown_secs: 60
  halfopen_trials: 5

providers:
  openai:
    provider: openai
    base_url: https://api.openai.com
  anthropic:
    provider: anthropic
    base_url: https://api.anthropic.com

pricing:
  dir: pricing

telemetry:
  json_logs: false
  metrics_enabled: true
"#;
