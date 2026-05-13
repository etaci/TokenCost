//! Error type used across the Fusebox crates.

use thiserror::Error;

pub type Result<T, E = FuseboxError> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum FuseboxError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("policy denied request: {reason}")]
    PolicyDenied { reason: String },

    #[error("circuit breaker is open for tenant {tenant}: {reason}")]
    BreakerOpen { tenant: String, reason: String },

    #[error("upstream request failed: {0}")]
    Upstream(String),

    #[error("pricing for model {model} not found")]
    UnknownModel { model: String },

    #[error("unsupported provider: {0}")]
    UnsupportedProvider(String),

    #[error("invalid request body: {0}")]
    InvalidRequest(String),

    #[error("storage error: {0}")]
    Storage(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("yaml error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("internal error: {0}")]
    Internal(String),
}

impl FuseboxError {
    /// Suggested HTTP status code for this error when surfaced through the
    /// proxy. Kept here so handlers can map errors uniformly.
    pub fn http_status(&self) -> u16 {
        match self {
            FuseboxError::PolicyDenied { .. } | FuseboxError::BreakerOpen { .. } => 429,
            FuseboxError::InvalidRequest(_) | FuseboxError::UnknownModel { .. } => 400,
            FuseboxError::UnsupportedProvider(_) => 404,
            FuseboxError::Upstream(_) => 502,
            _ => 500,
        }
    }
}
