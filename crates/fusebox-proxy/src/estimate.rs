//! Pre-flight cost estimation.
//!
//! Counts input tokens locally so the policy engine can deny *before* we
//! burn any upstream tokens. Tokenizers are provider-specific:
//!
//! - **OpenAI**: `tiktoken-rs` cl100k_base (gpt-3.5/4) and o200k_base (gpt-4o)
//! - **Anthropic / others**: no exact tokenizer available offline, fall back
//!   to a 4-chars-per-token heuristic. The estimate is conservative enough
//!   for budget gating; post-flight reconciliation uses real `usage` from
//!   the upstream response.

use fusebox_core::{CostUsd, ModelId, PricingTable, Provider, TokenUsage};
use serde::Deserialize;
use serde_json::Value;
use tiktoken_rs::{cl100k_base, o200k_base, CoreBPE};

/// Estimate the input-token count for an OpenAI chat-completions style
/// request body.
#[derive(Debug, Default, Deserialize)]
struct ChatCompletionsBody {
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub messages: Vec<Message>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub max_completion_tokens: Option<u32>,
    #[serde(default)]
    pub stream: bool,
}

#[derive(Debug, Default, Deserialize)]
struct Message {
    #[serde(default)]
    pub role: String,
    /// `content` may be a string or an array of content parts. We coerce to
    /// a single concatenated string for tokenization purposes.
    #[serde(default)]
    pub content: Value,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct AnthropicMessagesBody {
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub system: Value,
    #[serde(default)]
    pub messages: Vec<Message>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub stream: bool,
}

/// Gemini `:generateContent` request body. The interesting fields:
///   - `model` lives in the URL path, not the body — callers pass it via
///     a custom `x-fusebox-model` header (the proxy sets it before parsing)
///     OR we infer from the path. Here we accept either `model` (custom
///     ergonomics) or fall back to the empty string and let the upstream
///     reject malformed requests.
///   - `contents[].parts[].text` is the chat content.
///   - `generationConfig.maxOutputTokens` is the equivalent of `max_tokens`.
#[derive(Debug, Default, Deserialize)]
struct GoogleGenerateBody {
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub contents: Vec<GooglePart>,
    #[serde(default)]
    pub system_instruction: Option<GooglePart>,
    #[serde(default, rename = "generationConfig")]
    pub generation_config: Option<GoogleGenerationConfig>,
    /// Google streams via a separate `:streamGenerateContent` endpoint;
    /// the proxy sets this flag on the parsed request when the route
    /// matched the streaming variant.
    #[serde(skip)]
    pub stream: bool,
}

#[derive(Debug, Default, Deserialize)]
struct GooglePart {
    #[serde(default)]
    pub parts: Vec<GoogleInnerPart>,
    #[serde(default)]
    pub role: String,
}

#[derive(Debug, Default, Deserialize)]
struct GoogleInnerPart {
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Default, Deserialize)]
struct GoogleGenerationConfig {
    #[serde(default, rename = "maxOutputTokens")]
    pub max_output_tokens: Option<u32>,
}

/// Result of inspecting a request body.
#[derive(Debug, Clone)]
pub struct ParsedRequest {
    pub model: ModelId,
    pub estimated_usage: TokenUsage,
    pub estimated_cost: CostUsd,
    pub is_streaming: bool,
}

/// Inspect an OpenAI-compatible chat completions request body.
pub fn parse_openai_chat(body: &[u8], pricing: &PricingTable) -> Result<ParsedRequest, String> {
    let parsed: ChatCompletionsBody = serde_json::from_slice(body)
        .map_err(|e| format!("invalid OpenAI request body: {e}"))?;
    if parsed.model.is_empty() {
        return Err("`model` field is required".to_string());
    }
    let model = ModelId::new(parsed.model.clone());
    let text = concat_messages(&parsed.messages);
    let input_tokens = count_openai_tokens(&parsed.model, &text);
    let output_budget = parsed
        .max_completion_tokens
        .or(parsed.max_tokens)
        .unwrap_or(default_output_budget(&parsed.model));
    let estimated_usage = TokenUsage {
        input_tokens,
        output_tokens: output_budget,
        ..Default::default()
    };
    let estimated_cost = pricing
        .cost_for(&model, &estimated_usage)
        .unwrap_or(CostUsd::zero());
    Ok(ParsedRequest {
        model,
        estimated_usage,
        estimated_cost,
        is_streaming: parsed.stream,
    })
}

/// Inspect an Anthropic `/v1/messages` request body.
pub fn parse_anthropic_messages(
    body: &[u8],
    pricing: &PricingTable,
) -> Result<ParsedRequest, String> {
    let parsed: AnthropicMessagesBody = serde_json::from_slice(body)
        .map_err(|e| format!("invalid Anthropic request body: {e}"))?;
    if parsed.model.is_empty() {
        return Err("`model` field is required".to_string());
    }
    let model = ModelId::new(parsed.model.clone());
    let mut text = stringify_value(&parsed.system);
    if !text.is_empty() {
        text.push('\n');
    }
    text.push_str(&concat_messages(&parsed.messages));
    let input_tokens = count_chars_tokens(&text);
    let output_budget = parsed.max_tokens.unwrap_or(1024);
    let estimated_usage = TokenUsage {
        input_tokens,
        output_tokens: output_budget,
        ..Default::default()
    };
    let estimated_cost = pricing
        .cost_for(&model, &estimated_usage)
        .unwrap_or(CostUsd::zero());
    Ok(ParsedRequest {
        model,
        estimated_usage,
        estimated_cost,
        is_streaming: parsed.stream,
    })
}

/// Inspect a Google Gemini `:generateContent` request body. We use the
/// char-based heuristic since Google's tokenizer isn't available offline.
/// The model name lives in the URL path (`/v1beta/models/<id>:generate…`);
/// callers should set `body.model` before handing the bytes here — the
/// proxy route does that as a small synthetic preprocessing step.
pub fn parse_google_generate_content(
    body: &[u8],
    pricing: &PricingTable,
) -> Result<ParsedRequest, String> {
    let parsed: GoogleGenerateBody = serde_json::from_slice(body)
        .map_err(|e| format!("invalid Google request body: {e}"))?;
    if parsed.model.is_empty() {
        return Err("`model` field is required (proxy injects from URL path)".to_string());
    }
    let model = ModelId::new(parsed.model.clone());

    let mut text = String::new();
    if let Some(sys) = &parsed.system_instruction {
        for p in &sys.parts {
            text.push_str(&p.text);
            text.push('\n');
        }
    }
    for c in &parsed.contents {
        if !c.role.is_empty() {
            text.push_str(&c.role);
            text.push(':');
            text.push(' ');
        }
        for p in &c.parts {
            text.push_str(&p.text);
        }
        text.push('\n');
    }
    let input_tokens = count_chars_tokens(&text);
    let output_budget = parsed
        .generation_config
        .and_then(|c| c.max_output_tokens)
        .unwrap_or(1024);
    let estimated_usage = TokenUsage {
        input_tokens,
        output_tokens: output_budget,
        ..Default::default()
    };
    let estimated_cost = pricing
        .cost_for(&model, &estimated_usage)
        .unwrap_or(CostUsd::zero());
    Ok(ParsedRequest {
        model,
        estimated_usage,
        estimated_cost,
        is_streaming: parsed.stream,
    })
}

fn concat_messages(messages: &[Message]) -> String {
    let mut buf = String::new();
    for m in messages {
        if !m.role.is_empty() {
            buf.push_str(&m.role);
            buf.push(':');
            buf.push(' ');
        }
        buf.push_str(&stringify_value(&m.content));
        if let Some(n) = &m.name {
            buf.push(' ');
            buf.push_str(n);
        }
        buf.push('\n');
    }
    buf
}

/// Coerce a JSON `content` field (string OR array of parts) to plain text.
fn stringify_value(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|p| match p {
                Value::String(s) => Some(s.clone()),
                Value::Object(map) => map
                    .get("text")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" "),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// 4-chars-per-token heuristic. Crude but conservative for budget gating.
fn count_chars_tokens(text: &str) -> u32 {
    let chars = text.chars().count() as u32;
    chars.div_ceil(4).max(1)
}

fn count_openai_tokens(model: &str, text: &str) -> u32 {
    if text.is_empty() {
        return 0;
    }
    match openai_bpe(model) {
        Some(bpe) => bpe.encode_with_special_tokens(text).len() as u32,
        None => count_chars_tokens(text),
    }
}

fn openai_bpe(model: &str) -> Option<CoreBPE> {
    // gpt-4o family uses o200k; older gpt-3.5 / gpt-4 uses cl100k.
    let lower = model.to_ascii_lowercase();
    let result = if lower.starts_with("gpt-4o") || lower.contains("o200k") {
        o200k_base()
    } else {
        cl100k_base()
    };
    result.ok()
}

fn default_output_budget(model: &str) -> u32 {
    let lower = model.to_ascii_lowercase();
    if lower.contains("mini") || lower.contains("haiku") {
        1024
    } else {
        2048
    }
}

/// Map a model name to its provider, used when the request didn't tell us.
pub fn provider_for_model(model: &ModelId, pricing: &PricingTable) -> Provider {
    pricing
        .get(model)
        .and_then(|p| p.provider)
        .unwrap_or(Provider::Unknown)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pricing() -> PricingTable {
        PricingTable::from_yaml_str(
            r#"
provider: openai
models:
  gpt-4o-mini:
    input_per_1m: 0.15
    output_per_1m: 0.60
  gpt-4-turbo:
    input_per_1m: 10.0
    output_per_1m: 30.0
"#,
        )
        .unwrap()
    }

    #[test]
    fn parses_openai_chat_body() {
        let body = br#"{"model":"gpt-4o-mini","messages":[{"role":"user","content":"hello world"}],"max_tokens":50}"#;
        let parsed = parse_openai_chat(body, &pricing()).unwrap();
        assert_eq!(parsed.model.as_str(), "gpt-4o-mini");
        assert!(parsed.estimated_usage.input_tokens > 0);
        assert_eq!(parsed.estimated_usage.output_tokens, 50);
        assert!(!parsed.is_streaming);
        assert!(parsed.estimated_cost.dollars() > 0.0);
    }

    #[test]
    fn picks_up_streaming_flag() {
        let body =
            br#"{"model":"gpt-4o-mini","messages":[{"role":"user","content":"x"}],"stream":true}"#;
        let parsed = parse_openai_chat(body, &pricing()).unwrap();
        assert!(parsed.is_streaming);
    }

    #[test]
    fn parses_content_parts_array() {
        let body = br#"{"model":"gpt-4o-mini","messages":[{"role":"user","content":[{"type":"text","text":"hello"}]}]}"#;
        let parsed = parse_openai_chat(body, &pricing()).unwrap();
        assert!(parsed.estimated_usage.input_tokens > 0);
    }

    #[test]
    fn anthropic_uses_char_estimate() {
        let body = br#"{"model":"claude-sonnet-4-5","messages":[{"role":"user","content":"hello"}],"max_tokens":256}"#;
        let parsed = parse_anthropic_messages(body, &pricing()).unwrap();
        assert_eq!(parsed.model.as_str(), "claude-sonnet-4-5");
        assert_eq!(parsed.estimated_usage.output_tokens, 256);
    }
}
