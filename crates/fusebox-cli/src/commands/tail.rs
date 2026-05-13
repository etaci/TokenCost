//! `fusebox tail` — print recent spend events directly from the ledger.

use anyhow::{anyhow, Result};
use chrono::{DateTime, Duration, Utc};
use clap::Args;
use fusebox_core::{Config, StorageConfig};
use fusebox_ledger::{MemoryLedger, PgLedger, SharedLedger, SpendQuery, SqliteLedger};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration as StdDuration;

#[derive(Debug, Args)]
pub struct TailArgs {
    /// How many events to fetch per poll.
    #[arg(long, default_value_t = 20)]
    pub limit: u32,

    /// Poll interval in milliseconds.
    #[arg(long, default_value_t = 1500)]
    pub interval_ms: u64,

    /// Filter by tenant (optional).
    #[arg(long)]
    pub tenant: Option<String>,

    /// Show only events newer than the proxy's startup. Defaults to a
    /// 60-minute lookback so you see recent history on first connect.
    #[arg(long, default_value_t = 60)]
    pub lookback_minutes: i64,
}

pub async fn run(args: TailArgs, config_path: Option<PathBuf>) -> Result<()> {
    let cfg = load_config(config_path.as_deref())?;
    let ledger = open_ledger(&cfg.storage).await?;

    // We poll forward in time using the timestamp of the most recent event
    // we've already printed. Initial cursor is "now − lookback" so the
    // first poll surfaces some history; afterwards we only ask for things
    // strictly newer than what we last printed, which is the bug fix:
    // the old version compared by id and would re-print every event whose
    // id wasn't equal to the literal *last* one seen.
    let mut cursor: DateTime<Utc> =
        Utc::now() - Duration::minutes(args.lookback_minutes.max(1));
    let interval = StdDuration::from_millis(args.interval_ms.max(200));
    println!("tailing ledger — Ctrl-C to stop");
    loop {
        let query = SpendQuery {
            tenant: args.tenant.clone().map(fusebox_core::TenantId::from),
            since: cursor,
            until: None,
            limit: Some(args.limit),
        };
        let mut events = ledger
            .list(&query)
            .await
            .map_err(|e| anyhow!(e.to_string()))?;
        // ledger returns newest-first; reverse for chronological tail-print.
        events.reverse();

        for ev in events {
            if ev.ts <= cursor {
                continue;
            }
            print_event(&ev);
            cursor = ev.ts;
        }

        tokio::time::sleep(interval).await;
    }
}

fn print_event(ev: &fusebox_ledger::SpendEvent) {
    println!(
        "{ts}  {tenant:<12}  {provider:<10}  {model:<28}  in={in_t:<6} out={out_t:<6}  ${cost:.6}  [{status}]",
        ts = ev.ts.format("%H:%M:%S"),
        tenant = ev.tenant_id.as_str(),
        provider = ev.provider.as_str(),
        model = ev.model.as_str(),
        in_t = ev.input_tokens,
        out_t = ev.output_tokens,
        cost = ev.cost_usd.dollars(),
        status = ev.status.as_str(),
    );
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
