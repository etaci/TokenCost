//! Trait surface for ledger backends.

use crate::breaker_event::BreakerEvent;
use crate::event::SpendEvent;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use fusebox_core::{CostUsd, Result, TenantId};
use std::sync::Arc;

/// Aggregated spend over a window. Returned by `totals_for`.
#[derive(Debug, Clone, Default)]
pub struct SpendTotals {
    pub cost: CostUsd,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub events: u64,
}

/// Filter for spend queries.
#[derive(Debug, Clone)]
pub struct SpendQuery {
    pub tenant: Option<TenantId>,
    pub since: DateTime<Utc>,
    pub until: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
}

impl SpendQuery {
    pub fn for_tenant_since(tenant: TenantId, since: DateTime<Utc>) -> Self {
        Self {
            tenant: Some(tenant),
            since,
            until: None,
            limit: None,
        }
    }

    /// Cross-tenant query — used by the dashboard "everything live" view
    /// and the `/v1/events` admin endpoint.
    pub fn all_since(since: DateTime<Utc>) -> Self {
        Self {
            tenant: None,
            since,
            until: None,
            limit: None,
        }
    }
}

/// Filter for breaker-event audit queries.
#[derive(Debug, Clone)]
pub struct BreakerEventQuery {
    pub tenant: Option<TenantId>,
    pub since: DateTime<Utc>,
    pub limit: Option<u32>,
}

impl BreakerEventQuery {
    pub fn since(since: DateTime<Utc>) -> Self {
        Self {
            tenant: None,
            since,
            limit: None,
        }
    }
}

/// All backends implement this. Methods are async because most real
/// backends do IO; the in-memory variant is async too for parity.
#[async_trait]
pub trait LedgerStore: std::fmt::Debug + Send + Sync + 'static {
    /// Insert a single spend event. Implementations should be safe to call
    /// from many tasks concurrently and should batch internally if needed.
    async fn record(&self, event: SpendEvent) -> Result<()>;

    /// Total cost / token counts matching `query`.
    async fn totals(&self, query: &SpendQuery) -> Result<SpendTotals>;

    /// Latest events matching `query`. Used by the dashboard live stream.
    async fn list(&self, query: &SpendQuery) -> Result<Vec<SpendEvent>>;

    /// Persist a circuit-breaker transition for the audit log. Default
    /// implementation is a no-op so legacy / test backends keep compiling.
    async fn record_breaker(&self, _event: BreakerEvent) -> Result<()> {
        Ok(())
    }

    /// Latest breaker transitions, newest first.
    async fn list_breaker_events(&self, _query: &BreakerEventQuery) -> Result<Vec<BreakerEvent>> {
        Ok(Vec::new())
    }

    /// Lifetime sanity check, called at startup. Return Ok(()) when the
    /// underlying store is reachable and migrations are applied.
    async fn ping(&self) -> Result<()>;
}

/// `Arc<dyn LedgerStore>` shorthand we hand around between crates.
pub type SharedLedger = Arc<dyn LedgerStore>;
