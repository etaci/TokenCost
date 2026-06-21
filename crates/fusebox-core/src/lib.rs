//! # fusebox-core
//!
//! Shared primitives used across every Fusebox crate.
//!
//! Nothing here touches IO. Network, database, and policy execution all live
//! in dedicated crates that depend on the types defined here.

#![deny(missing_debug_implementations)]
#![warn(rust_2018_idioms)]

pub mod budget;
pub mod config;
pub mod decision;
pub mod error;
pub mod pricing;
pub mod tenant;
pub mod usage;

pub use budget::{Budget, BudgetWindow};
pub use config::{Config, ProviderConfig, ProxyConfig, StorageConfig};
pub use decision::{Decision, DenyReason};
pub use error::{FuseboxError, Result};
pub use pricing::{ModelPricing, PricingTable};
pub use tenant::TenantId;
pub use usage::{CostUsd, ModelId, Provider, TokenUsage};
