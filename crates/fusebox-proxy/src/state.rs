//! Shared application state passed to every handler.

use std::sync::Arc;

use fusebox_core::{Config, PricingTable};
use fusebox_ledger::{SharedLedger, SpendEvent};
use fusebox_policy::SharedPolicy;
use parking_lot::RwLock;
use reqwest::Client;
use tokio::sync::broadcast;

use crate::budget_requests::BudgetRequestStore;

/// Capacity of the live-events broadcast channel. Slow subscribers that lag
/// behind by more than this many events get a `Lagged` notification and miss
/// the gap — fine for a live tail since the gap is also visible in the
/// ledger via `/v1/events`.
const EVENT_BROADCAST_CAPACITY: usize = 512;

/// Cheap copy-on-write handle around an `Arc<T>`. Reads clone the Arc
/// (single atomic op); writers replace the inner Arc atomically via the
/// `RwLock`. Used so the admin "reload config" path can swap the pricing
/// table or app config under live traffic without coordinating with every
/// request in flight.
#[derive(Debug)]
pub struct Reloadable<T> {
    inner: RwLock<Arc<T>>,
}

impl<T> Reloadable<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: RwLock::new(Arc::new(value)),
        }
    }

    /// Snapshot the current value. The returned Arc may outlive a
    /// concurrent `replace`; that's intentional — handlers see a stable
    /// view for the duration of their request.
    pub fn load(&self) -> Arc<T> {
        self.inner.read().clone()
    }

    pub fn replace(&self, value: T) {
        *self.inner.write() = Arc::new(value);
    }
}

/// Bundled handles every handler needs. Cheap to clone — everything is
/// either an `Arc` or a connection-pooled client.
#[derive(Clone)]
pub struct AppState {
    /// Loaded `fusebox.yaml` (or compiled defaults). Reloadable so the
    /// admin endpoint / SIGHUP can swap config without a restart.
    pub config: Arc<Reloadable<Config>>,
    /// The pricing table; same swap semantics as `config`.
    pub pricing: Arc<Reloadable<PricingTable>>,
    pub ledger: SharedLedger,
    pub policy: SharedPolicy,
    pub http: Client,
    /// Broadcast bus the proxy publishes every recorded spend event to.
    /// Subscribers come from `/v1/events/stream` (SSE) and future MCP
    /// agents.  Bounded so a stuck consumer can never grow memory.
    pub events_tx: broadcast::Sender<SpendEvent>,
    /// In-memory queue of pending and decided budget-increase requests.
    pub budget_requests: BudgetRequestStore,
    /// Where the config was loaded from, so admin/reload can re-read.
    /// `None` when no on-disk config was found (we booted from defaults).
    pub config_path: Arc<RwLock<Option<std::path::PathBuf>>>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("config", &self.config.load())
            .field("pricing_models", &self.pricing.load().len())
            .finish_non_exhaustive()
    }
}

impl AppState {
    pub fn new(
        config: Config,
        pricing: PricingTable,
        ledger: SharedLedger,
        policy: SharedPolicy,
    ) -> Self {
        let timeout = std::time::Duration::from_secs(config.proxy.upstream_timeout_secs);
        let http = Client::builder()
            .pool_idle_timeout(std::time::Duration::from_secs(60))
            .timeout(timeout)
            .build()
            .expect("reqwest client must build");
        let (events_tx, _) = broadcast::channel(EVENT_BROADCAST_CAPACITY);
        Self {
            config: Arc::new(Reloadable::new(config)),
            pricing: Arc::new(Reloadable::new(pricing)),
            ledger,
            policy,
            http,
            events_tx,
            budget_requests: BudgetRequestStore::new(),
            config_path: Arc::new(RwLock::new(None)),
        }
    }

    pub fn set_config_path(&self, path: std::path::PathBuf) {
        *self.config_path.write() = Some(path);
    }

    /// Best-effort publish. Never fails: when there are no subscribers
    /// `broadcast::send` returns `Err(SendError)` which we ignore.
    pub fn publish_event(&self, event: SpendEvent) {
        let _ = self.events_tx.send(event);
    }
}
