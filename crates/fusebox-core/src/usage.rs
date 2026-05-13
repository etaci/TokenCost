//! Token usage and provider-agnostic cost types.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Provider identifier. Kept as an enum (not free-form string) so we can
/// pattern-match on it in the proxy and serialize it stably to the ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    OpenAI,
    Anthropic,
    Bedrock,
    Google,
    OpenRouter,
    Unknown,
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Provider::OpenAI => "openai",
            Provider::Anthropic => "anthropic",
            Provider::Bedrock => "bedrock",
            Provider::Google => "google",
            Provider::OpenRouter => "openrouter",
            Provider::Unknown => "unknown",
        }
    }
}

impl fmt::Display for Provider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&str> for Provider {
    fn from(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "openai" => Provider::OpenAI,
            "anthropic" => Provider::Anthropic,
            "bedrock" => Provider::Bedrock,
            "google" => Provider::Google,
            "openrouter" => Provider::OpenRouter,
            _ => Provider::Unknown,
        }
    }
}

/// A model name as the user wrote it (e.g. `gpt-4o-mini`,
/// `claude-sonnet-4-5`). Pricing tables key off this raw string so we don't
/// hardcode an enum that goes stale every month.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModelId(pub String);

impl ModelId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ModelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for ModelId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for ModelId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Token counts for a single LLM call. Mirrors what providers return in the
/// `usage` field of their responses.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    /// Cached input tokens (Anthropic / OpenAI cache) — billed separately.
    #[serde(default)]
    pub cache_read_tokens: u32,
    #[serde(default)]
    pub cache_write_tokens: u32,
}

impl TokenUsage {
    pub fn total(&self) -> u32 {
        self.input_tokens + self.output_tokens
    }

    pub fn merge(&mut self, other: &TokenUsage) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.cache_read_tokens += other.cache_read_tokens;
        self.cache_write_tokens += other.cache_write_tokens;
    }
}

/// Cost expressed in USD. Stored as f64 because we never need more than
/// 6 decimals of precision and string math would just bloat the hot path.
/// Persisted to the ledger as `NUMERIC(12,6)` to avoid float drift on disk.
#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CostUsd(pub f64);

impl CostUsd {
    pub fn zero() -> Self {
        Self(0.0)
    }

    pub fn dollars(&self) -> f64 {
        self.0
    }

    pub fn cents(&self) -> f64 {
        self.0 * 100.0
    }
}

impl std::ops::Add for CostUsd {
    type Output = CostUsd;
    fn add(self, rhs: CostUsd) -> CostUsd {
        CostUsd(self.0 + rhs.0)
    }
}

impl std::ops::AddAssign for CostUsd {
    fn add_assign(&mut self, rhs: CostUsd) {
        self.0 += rhs.0;
    }
}

impl fmt::Display for CostUsd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "${:.6}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_roundtrip() {
        for p in [
            Provider::OpenAI,
            Provider::Anthropic,
            Provider::Bedrock,
            Provider::Google,
            Provider::OpenRouter,
        ] {
            assert_eq!(Provider::from(p.as_str()), p);
        }
        assert_eq!(Provider::from("UnknownVendor"), Provider::Unknown);
    }

    #[test]
    fn token_usage_total_and_merge() {
        let mut a = TokenUsage {
            input_tokens: 10,
            output_tokens: 5,
            ..Default::default()
        };
        let b = TokenUsage {
            input_tokens: 3,
            output_tokens: 7,
            ..Default::default()
        };
        a.merge(&b);
        assert_eq!(a.total(), 25);
    }

    #[test]
    fn cost_addition() {
        let total = CostUsd(0.5) + CostUsd(0.25);
        assert!((total.0 - 0.75).abs() < f64::EPSILON);
    }
}
