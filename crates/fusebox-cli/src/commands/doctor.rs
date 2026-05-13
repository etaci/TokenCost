//! `fusebox doctor` — sanity-check ledger / pricing / config.

use anyhow::{anyhow, Result};
use chrono::{Duration, Utc};
use clap::Args;
use fusebox_core::{Config, PricingTable, StorageConfig, TenantId};
use fusebox_ledger::{MemoryLedger, PgLedger, SharedLedger, SpendQuery, SqliteLedger};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Args)]
pub struct DoctorArgs {
    /// Tenant to use when test-querying the ledger.
    #[arg(long, default_value = "default")]
    pub tenant: String,
}

pub async fn run(args: DoctorArgs, config_path: Option<PathBuf>) -> Result<()> {
    let cfg = load_config(config_path.as_deref())?;
    println!("== fusebox doctor ==");

    // 1. Config + bind reachability
    println!("config.bind                   : {}", cfg.proxy.bind);
    println!("config.upstream_timeout_secs  : {}", cfg.proxy.upstream_timeout_secs);
    println!("config.providers              : {} entries", cfg.providers.len());

    // 2. Pricing table
    match &cfg.pricing.dir {
        Some(dir) if dir.exists() => match PricingTable::load_dir(dir) {
            Ok(t) => println!("pricing.dir                   : {} ({} models)", dir.display(), t.len()),
            Err(e) => println!("pricing.dir                   : ERROR — {e}"),
        },
        Some(dir) => println!("pricing.dir                   : {} (missing — estimates default to $0)", dir.display()),
        None => println!("pricing.dir                   : (none)"),
    }

    // 3. Ledger
    let ledger = open_ledger(&cfg.storage).await?;
    match ledger.ping().await {
        Ok(()) => println!("ledger.ping                   : ok"),
        Err(e) => println!("ledger.ping                   : ERROR — {e}"),
    }

    let q = SpendQuery::for_tenant_since(
        TenantId::from(args.tenant.as_str()),
        Utc::now() - Duration::days(1),
    );
    match ledger.totals(&q).await {
        Ok(t) => println!(
            "ledger.totals[{}, 1d]      : ${:.4} across {} events",
            args.tenant, t.cost.dollars(), t.events
        ),
        Err(e) => println!("ledger.totals                 : ERROR — {e}"),
    }

    Ok(())
}

fn load_config(path: Option<&std::path::Path>) -> Result<Config> {
    if let Some(p) = path {
        return Config::from_yaml_file(p).map_err(|e| anyhow!(e.to_string()));
    }
    let cwd = std::path::Path::new("fusebox.yaml");
    if cwd.exists() {
        return Config::from_yaml_file(cwd).map_err(|e| anyhow!(e.to_string()));
    }
    Ok(Config::default())
}

async fn open_ledger(storage: &StorageConfig) -> Result<SharedLedger> {
    let ledger: SharedLedger = match storage {
        StorageConfig::Sqlite { path } => Arc::new(
            SqliteLedger::connect(path)
                .await
                .map_err(|e| anyhow!(e.to_string()))?,
        ),
        StorageConfig::Memory => Arc::new(MemoryLedger::new()),
        StorageConfig::Postgres { url } => Arc::new(
            PgLedger::connect(url)
                .await
                .map_err(|e| anyhow!(e.to_string()))?,
        ),
    };
    Ok(ledger)
}
