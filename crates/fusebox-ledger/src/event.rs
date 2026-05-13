//! The shape of a single spend row, before it ever reaches a database.

use chrono::{DateTime, Utc};
use fusebox_core::{CostUsd, ModelId, Provider, TenantId, TokenUsage};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpendEvent {
    pub id: Uuid,
    pub ts: DateTime<Utc>,
    pub tenant_id: TenantId,
    pub provider: Provider,
    pub model: ModelId,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_read_tokens: u32,
    pub cache_write_tokens: u32,
    pub cost_usd: CostUsd,
    pub request_id: Option<String>,
    pub status: SpendStatus,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpendStatus {
    /// Request was forwarded and the upstream returned a usable response.
    Completed,
    /// Request was denied or downgraded by Fusebox before reaching upstream.
    Blocked,
    /// Forwarded but the upstream failed; cost may be zero or partial.
    Failed,
}

impl SpendStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            SpendStatus::Completed => "completed",
            SpendStatus::Blocked => "blocked",
            SpendStatus::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "blocked" => SpendStatus::Blocked,
            "failed" => SpendStatus::Failed,
            _ => SpendStatus::Completed,
        }
    }
}

impl SpendEvent {
    /// Convenience constructor used by the proxy. The id and timestamp are
    /// generated here so callers don't have to remember to do it.
    pub fn now(
        tenant_id: TenantId,
        provider: Provider,
        model: ModelId,
        usage: TokenUsage,
        cost_usd: CostUsd,
        status: SpendStatus,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            ts: Utc::now(),
            tenant_id,
            provider,
            model,
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            cache_read_tokens: usage.cache_read_tokens,
            cache_write_tokens: usage.cache_write_tokens,
            cost_usd,
            request_id: None,
            status,
            metadata: serde_json::Value::Null,
        }
    }

    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }
}
