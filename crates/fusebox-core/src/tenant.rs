//! Identifiers used to scope budgets, breakers, and ledger rows.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A tenant is the smallest billable unit Fusebox knows about. It maps 1:1
/// to a circuit breaker and at least one budget. The exact mapping (user,
/// project, team) is up to the deployer.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TenantId(pub String);

impl TenantId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Default tenant used when a request arrives with no identifying
    /// header. Useful for the indie / single-user flow.
    pub fn default_tenant() -> Self {
        Self("default".to_string())
    }
}

impl fmt::Display for TenantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for TenantId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for TenantId {
    fn from(value: String) -> Self {
        Self(value)
    }
}
