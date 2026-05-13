//! High-level "boot the whole proxy" entrypoint.
//!
//! Wires together: config loading → ledger backend → policy engine →
//! pricing table → axum router → tcp listener.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context};
use fusebox_core::{Config, FuseboxError, PricingTable, StorageConfig};
use fusebox_ledger::{MemoryLedger, PgLedger, SharedLedger, SqliteLedger};
use fusebox_policy::engine::{PolicyConfig as EnginePolicyConfig, PolicyEngine};
use tokio::net::TcpListener;
use tracing::{info, warn};

use crate::metrics::install as install_metrics;
use crate::routes::router;
use crate::state::AppState;

/// Knobs for `run` that aren't worth threading through `Config`.
#[derive(Debug, Default, Clone)]
pub struct RunOptions {
    /// Optional explicit config file. When `None`, falls back to the
    /// layered loader (`./fusebox.yaml`, env, defaults).
    pub config_path: Option<PathBuf>,
    /// Override the bind address from the CLI.
    pub bind_override: Option<String>,
}

/// Top-level boot. Resolves config + dependencies, then hands control to
/// `run_with_listener`.
pub async fn run(opts: RunOptions) -> anyhow::Result<()> {
    let resolved = resolve_config_path(opts.config_path.as_deref());
    let config = match &resolved {
        Some(p) => Config::from_yaml_file(p).map_err(|e| anyhow!(e.to_string()))?,
        None => Config::default(),
    };
    let bind = opts
        .bind_override
        .unwrap_or_else(|| config.proxy.bind.clone());
    let addr: SocketAddr = bind
        .parse()
        .with_context(|| format!("invalid bind address: {bind}"))?;
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind {addr}"))?;
    info!("fusebox proxy listening on http://{addr}");
    run_with_listener_and_path(listener, config, resolved).await
}

/// Useful for tests: caller owns the listener (e.g. ephemeral port).
pub async fn run_with_listener(listener: TcpListener, config: Config) -> anyhow::Result<()> {
    run_with_listener_and_path(listener, config, None).await
}

async fn run_with_listener_and_path(
    listener: TcpListener,
    config: Config,
    config_path: Option<PathBuf>,
) -> anyhow::Result<()> {
    install_metrics();

    let ledger = build_ledger(&config.storage).await?;
    ledger.ping().await.map_err(|e| anyhow!(e.to_string()))?;

    let pricing = build_pricing(&config)?;
    info!(models = pricing.len(), "pricing table loaded");

    let engine_cfg = EnginePolicyConfig::from_core(&config.policy);
    let policy = Arc::new(PolicyEngine::new(engine_cfg, ledger.clone()));

    let state = AppState::new(config, pricing, ledger, policy);
    if let Some(p) = &config_path {
        state.set_config_path(p.clone());
    }

    // SIGHUP listener (Unix only) — re-run the same disk read + swap that
    // `/v1/admin/reload` does. On Windows there's no SIGHUP, but operators
    // can hit the admin endpoint over loopback to get equivalent behaviour.
    #[cfg(unix)]
    spawn_sighup_listener(state.clone());

    let app = router(state);

    axum::serve(listener, app.into_make_service())
        .await
        .map_err(|e| anyhow!("axum serve failed: {e}"))?;
    Ok(())
}

#[cfg(unix)]
fn spawn_sighup_listener(state: AppState) {
    use tokio::signal::unix::{signal, SignalKind};
    tokio::spawn(async move {
        let mut sig = match signal(SignalKind::hangup()) {
            Ok(s) => s,
            Err(e) => {
                warn!("failed to install SIGHUP listener: {e}");
                return;
            }
        };
        loop {
            if sig.recv().await.is_none() {
                break;
            }
            info!("SIGHUP received — reloading config");
            match crate::admin::reload(&state).await {
                Ok(summary) => info!(?summary, "reload complete"),
                Err(e) => warn!("reload failed: {e}"),
            }
        }
    });
}

fn resolve_config_path(explicit: Option<&std::path::Path>) -> Option<PathBuf> {
    if let Some(p) = explicit {
        return Some(p.to_path_buf());
    }
    let cwd_path = std::path::Path::new("fusebox.yaml");
    if cwd_path.exists() {
        return Some(cwd_path.to_path_buf());
    }
    None
}

#[allow(dead_code)] // kept for back-compat with external callers that imported it
fn load_config(explicit: Option<&std::path::Path>) -> anyhow::Result<Config> {
    match resolve_config_path(explicit) {
        Some(p) => Config::from_yaml_file(&p).map_err(|e| anyhow!(e.to_string())),
        None => Ok(Config::default()),
    }
}

async fn build_ledger(storage: &StorageConfig) -> anyhow::Result<SharedLedger> {
    let ledger: SharedLedger = match storage {
        StorageConfig::Sqlite { path } => {
            info!(path = %path.display(), "using sqlite ledger");
            Arc::new(
                SqliteLedger::connect(path)
                    .await
                    .map_err(|e: FuseboxError| anyhow!(e.to_string()))?,
            )
        }
        StorageConfig::Memory => {
            warn!("using in-memory ledger — spend will not survive restarts");
            Arc::new(MemoryLedger::new())
        }
        StorageConfig::Postgres { url } => {
            info!("using postgres ledger");
            Arc::new(
                PgLedger::connect(url)
                    .await
                    .map_err(|e: FuseboxError| anyhow!(e.to_string()))?,
            )
        }
    };
    Ok(ledger)
}

fn build_pricing(config: &Config) -> anyhow::Result<PricingTable> {
    let dir = match &config.pricing.dir {
        Some(d) => d.clone(),
        None => return Ok(PricingTable::new()),
    };
    if !dir.exists() {
        warn!(
            dir = %dir.display(),
            "pricing directory missing — proxy will run but cost estimates default to $0"
        );
        return Ok(PricingTable::new());
    }
    PricingTable::load_dir(&dir).map_err(|e| anyhow!(e.to_string()))
}
