//! # fusebox-policy
//!
//! The brain of Fusebox. Combines budget arithmetic with a circuit-breaker
//! state machine to make a single per-request decision: *Allow*, *Deny*,
//! or *Downgrade*.
//!
//! Everything here is in-process and lock-light (DashMap shards). The
//! ledger is the source of truth for spend; we cache rolling totals in
//! memory to keep the hot path away from disk.

#![warn(rust_2018_idioms)]

pub mod anomaly;
pub mod breaker;
pub mod engine;
pub mod estimate;

pub use anomaly::{AnomalyVerdict, EwmaDetector};
pub use breaker::{Breaker, BreakerLabel, BreakerState, BreakerTransition};
pub use engine::{PolicyEngine, SharedPolicy};
pub use estimate::RequestEstimate;
