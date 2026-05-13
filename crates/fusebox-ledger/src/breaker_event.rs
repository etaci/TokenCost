//! Audit-log entry for circuit-breaker state transitions.
//!
//! Every time a breaker flips state — automatic trip, automatic recovery,
//! or operator override — we drop a row in here. This is the answer to
//! "why did Fusebox start denying my traffic at 02:14?"

use chrono::{DateTime, Utc};
use fusebox_core::TenantId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BreakerTransitionKind {
    Trip,
    HalfOpen,
    Close,
    ManualReset,
}

impl BreakerTransitionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            BreakerTransitionKind::Trip => "trip",
            BreakerTransitionKind::HalfOpen => "half_open",
            BreakerTransitionKind::Close => "close",
            BreakerTransitionKind::ManualReset => "manual_reset",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "trip" => BreakerTransitionKind::Trip,
            "half_open" => BreakerTransitionKind::HalfOpen,
            "manual_reset" => BreakerTransitionKind::ManualReset,
            _ => BreakerTransitionKind::Close,
        }
    }
}

/// One row in the `breaker_events` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakerEvent {
    pub id: Uuid,
    pub ts: DateTime<Utc>,
    pub tenant_id: TenantId,
    pub transition: BreakerTransitionKind,
    /// Free-form reason ("budget_exceeded(1d)", "anomaly(z=4.2)", "manual").
    pub reason: Option<String>,
}

impl BreakerEvent {
    pub fn now(
        tenant_id: TenantId,
        transition: BreakerTransitionKind,
        reason: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            ts: Utc::now(),
            tenant_id,
            transition,
            reason,
        }
    }
}
