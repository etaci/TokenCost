//! # fusebox-ledger
//!
//! Persistence for spend events. Two production backends are supported:
//!
//! - **SQLite** — zero-config default, perfect for indie / single-node use.
//! - **Postgres** — recommended once you outgrow a single box; pairs with
//!   TimescaleDB for time-series rollups.
//!
//! Both implement the [`LedgerStore`] trait so the policy engine never
//! needs to know which one is plugged in.

#![warn(rust_2018_idioms)]

pub mod breaker_event;
pub mod event;
pub mod memory;
pub mod postgres;
pub mod sqlite;
pub mod store;

pub use breaker_event::{BreakerEvent, BreakerTransitionKind};
pub use event::SpendEvent;
pub use memory::MemoryLedger;
pub use postgres::PgLedger;
pub use sqlite::SqliteLedger;
pub use store::{BreakerEventQuery, LedgerStore, SharedLedger, SpendQuery, SpendTotals};
