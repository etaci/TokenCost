//! Cost estimate produced by the proxy before forwarding a request.
//!
//! The estimate gates the pre-flight check: if `current_spend + estimated
//! > budget`, we deny *before* burning any upstream tokens.

use fusebox_core::{CostUsd, ModelId, Provider, TokenUsage};

#[derive(Debug, Clone)]
pub struct RequestEstimate {
    pub provider: Provider,
    pub model: ModelId,
    /// Best guess at input tokens, computed locally with the provider's
    /// tokenizer. Output is unknowable up-front; we use a conservative
    /// projection (`max_tokens` from the request body, falling back to a
    /// model-specific default).
    pub estimated_usage: TokenUsage,
    /// Estimated dollar cost of `estimated_usage`.
    pub estimated_cost: CostUsd,
    /// Whether the upstream call will stream. Streaming responses still
    /// roll into the same ledger row but require post-flight reconciliation.
    pub is_streaming: bool,
}

impl RequestEstimate {
    pub fn new(
        provider: Provider,
        model: ModelId,
        estimated_usage: TokenUsage,
        estimated_cost: CostUsd,
    ) -> Self {
        Self {
            provider,
            model,
            estimated_usage,
            estimated_cost,
            is_streaming: false,
        }
    }

    pub fn streaming(mut self, is_streaming: bool) -> Self {
        self.is_streaming = is_streaming;
        self
    }
}
