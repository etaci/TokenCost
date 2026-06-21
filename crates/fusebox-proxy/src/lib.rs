//! # fusebox-proxy
//!
//! OpenAI / Anthropic compatible HTTP gateway. Every upstream LLM call goes
//! through here so the policy engine can pre-flight gate it, the ledger can
//! account for it, and the circuit breaker can shut it down if a budget
//! blows.
//!
//! The crate ships a `main.rs` binary, but everything useful is exposed
//! via this library so the `fusebox-cli` `start` subcommand can boot the
//! proxy without forking a process.

#![warn(rust_2018_idioms)]

pub mod admin;
pub mod app;
pub mod budget_requests;
pub mod estimate;
pub mod identity;
pub mod metrics;
pub mod routes;
pub mod state;
pub mod stream_reconcile;
pub mod upstream;

pub use app::{run, run_with_listener, RunOptions};
pub use state::AppState;
