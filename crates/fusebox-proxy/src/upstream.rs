//! Forward HTTP requests to upstream LLM providers and pull `usage` numbers
//! out of the response so the ledger can record real spend.
//!
//! Two response shapes:
//!
//! - **OpenAI / OpenRouter**: `{"usage": {"prompt_tokens", "completion_tokens", ...}}`
//! - **Anthropic**: `{"usage": {"input_tokens", "output_tokens", ...}}`

use bytes::Bytes;
use fusebox_core::{Provider, TokenUsage};
use reqwest::header::{HeaderMap, HeaderName, CONTENT_LENGTH, HOST};
use reqwest::{Client, Method, Response, Url};
use serde::Deserialize;

// `override_authorization` was removed: nothing inside the proxy crate
// rewrote the upstream Authorization header (we always pass it through),
// and the helper referenced an un-imported `HeaderValue` which broke the
// build. If/when we add proxy-managed keys, the helper should live next
// to the credential store, not in the bare HTTP-forward layer.

/// Headers we strip when forwarding because they refer to the *hop*, not
/// the request.
fn hop_by_hop_or_host(name: &HeaderName) -> bool {
    matches!(
        name.as_str(),
        "host"
            | "content-length"
            | "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailers"
            | "transfer-encoding"
            | "upgrade"
    ) || name == HOST
        || name == CONTENT_LENGTH
}

fn sanitize_outgoing(headers: &HeaderMap) -> HeaderMap {
    let mut out = HeaderMap::new();
    for (k, v) in headers {
        if hop_by_hop_or_host(k) {
            continue;
        }
        // Strip our own internal headers — they shouldn't reach the upstream.
        if k.as_str().starts_with("x-fusebox-") {
            continue;
        }
        out.insert(k.clone(), v.clone());
    }
    out
}

/// Forward a request to the upstream and return the raw response. The
/// caller decides whether to read the body fully or stream it.
pub async fn forward(
    client: &Client,
    base_url: &str,
    path_and_query: &str,
    method: Method,
    headers: &HeaderMap,
    body: Bytes,
) -> Result<Response, String> {
    let url = build_url(base_url, path_and_query)?;
    let req = client
        .request(method, url)
        .headers(sanitize_outgoing(headers))
        .body(body)
        .build()
        .map_err(|e| format!("build upstream request: {e}"))?;
    client
        .execute(req)
        .await
        .map_err(|e| format!("upstream call failed: {e}"))
}

fn build_url(base: &str, path_and_query: &str) -> Result<Url, String> {
    // base like "https://api.openai.com" + "/v1/chat/completions?x=y"
    let trimmed = base.trim_end_matches('/');
    let suffix = if path_and_query.starts_with('/') {
        path_and_query.to_string()
    } else {
        format!("/{path_and_query}")
    };
    Url::parse(&format!("{trimmed}{suffix}")).map_err(|e| format!("bad upstream URL: {e}"))
}

/// Parse `usage` out of a buffered upstream response body. Returns
/// `TokenUsage::default()` if we can't find one — the caller falls back to
/// the pre-flight estimate in that case.
#[derive(Debug, Default, Deserialize)]
struct OpenAiUsage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
    #[serde(default)]
    #[allow(dead_code)]
    total_tokens: u32,
    #[serde(default)]
    cached_tokens: u32,
    #[serde(default)]
    prompt_tokens_details: Option<PromptTokensDetails>,
}

#[derive(Debug, Default, Deserialize)]
struct PromptTokensDetails {
    #[serde(default)]
    cached_tokens: u32,
}

#[derive(Debug, Default, Deserialize)]
struct AnthropicUsage {
    #[serde(default)]
    input_tokens: u32,
    #[serde(default)]
    output_tokens: u32,
    #[serde(default)]
    cache_read_input_tokens: u32,
    #[serde(default)]
    cache_creation_input_tokens: u32,
}

pub fn parse_usage(body: &[u8], provider: Provider) -> TokenUsage {
    let v: serde_json::Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) => return TokenUsage::default(),
    };
    let usage = match v.get("usage") {
        Some(u) => u,
        None => return TokenUsage::default(),
    };
    match provider {
        Provider::Anthropic => {
            let parsed: AnthropicUsage = serde_json::from_value(usage.clone()).unwrap_or_default();
            TokenUsage {
                input_tokens: parsed.input_tokens,
                output_tokens: parsed.output_tokens,
                cache_read_tokens: parsed.cache_read_input_tokens,
                cache_write_tokens: parsed.cache_creation_input_tokens,
            }
        }
        _ => {
            let parsed: OpenAiUsage = serde_json::from_value(usage.clone()).unwrap_or_default();
            let cached = parsed
                .prompt_tokens_details
                .map(|d| d.cached_tokens)
                .unwrap_or(parsed.cached_tokens);
            TokenUsage {
                input_tokens: parsed.prompt_tokens.saturating_sub(cached),
                output_tokens: parsed.completion_tokens,
                cache_read_tokens: cached,
                cache_write_tokens: 0,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_openai_usage() {
        let body = br#"{"id":"x","usage":{"prompt_tokens":120,"completion_tokens":35,"total_tokens":155,"prompt_tokens_details":{"cached_tokens":20}}}"#;
        let usage = parse_usage(body, Provider::OpenAI);
        // 120 prompt − 20 cached = 100 effective input
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 35);
        assert_eq!(usage.cache_read_tokens, 20);
    }

    #[test]
    fn parses_anthropic_usage() {
        let body = br#"{"id":"y","usage":{"input_tokens":42,"output_tokens":17,"cache_read_input_tokens":5,"cache_creation_input_tokens":2}}"#;
        let usage = parse_usage(body, Provider::Anthropic);
        assert_eq!(usage.input_tokens, 42);
        assert_eq!(usage.output_tokens, 17);
        assert_eq!(usage.cache_read_tokens, 5);
        assert_eq!(usage.cache_write_tokens, 2);
    }

    #[test]
    fn missing_usage_returns_default() {
        let body = br#"{"id":"z","choices":[]}"#;
        let usage = parse_usage(body, Provider::OpenAI);
        assert_eq!(usage, TokenUsage::default());
    }

    #[test]
    fn join_url_correctly() {
        let url = build_url("https://api.openai.com/", "/v1/chat/completions?stream=1").unwrap();
        assert_eq!(
            url.as_str(),
            "https://api.openai.com/v1/chat/completions?stream=1"
        );
    }
}
