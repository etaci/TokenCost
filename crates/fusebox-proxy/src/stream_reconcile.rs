//! Server-sent-event reconciliation for streaming LLM responses.
//!
//! The proxy passes bytes through to the client unchanged but also fans
//! every chunk into an `mpsc` channel; this module owns the consumer side.
//! It buffers chunks, splits them on the SSE event boundary (`\n\n` /
//! `\r\n\r\n`), parses the `data:` payload as JSON, and folds usage numbers
//! out of OpenAI / Anthropic-style stream events.
//!
//! Why bother instead of trusting the pre-flight estimate?
//! - OpenAI emits the final `usage` block in the **last** streamed chunk
//!   when the caller sets `stream_options: { include_usage: true }`.
//! - Anthropic emits `usage.input_tokens` in `message_start` and
//!   `usage.output_tokens` in `message_delta` — both are authoritative.
//!
//! If we never see a usage block (older OpenAI clients, exotic providers),
//! we fall back to the pre-flight estimate so the breaker still moves.
//!
//! Robustness:
//! - The mpsc receiver is unbounded; a stuck consumer can't exert
//!   back-pressure on the client-facing stream, which would stall the user.
//! - We treat decode failures as "skip this event, keep going" — partial
//!   data is normal mid-stream.
//! - On client disconnect, the sender side drops; we still finalise with
//!   the partial usage we have, which is what the user actually consumed.

use bytes::Bytes;
use fusebox_core::TokenUsage;
use serde_json::Value;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamFlavor {
    /// OpenAI / OpenAI-compatible / OpenRouter — `data: {...}\n\n` with
    /// `usage` only in the final chunk when `include_usage` is on.
    OpenAi,
    /// Anthropic — multi-event SSE with `event:` and `data:` lines;
    /// `usage.input_tokens` lands in `message_start`, `output_tokens`
    /// in `message_delta`, possibly with cache deltas.
    Anthropic,
}

/// Consume an SSE byte stream off a channel and return the final usage.
/// When nothing was extractable, returns `TokenUsage::default()` and the
/// caller is expected to fall back to its pre-flight estimate.
pub async fn reconcile(
    mut rx: mpsc::UnboundedReceiver<Bytes>,
    flavor: StreamFlavor,
) -> TokenUsage {
    let mut buf: Vec<u8> = Vec::new();
    let mut usage = TokenUsage::default();
    while let Some(chunk) = rx.recv().await {
        buf.extend_from_slice(&chunk);
        while let Some(end) = next_event_end(&buf) {
            // `end` is the index immediately after the terminating blank
            // line. Drain the event bytes; leave the rest for the next pass.
            let event: Vec<u8> = buf.drain(..end).collect();
            process_event(&event, flavor, &mut usage);
        }
    }
    // Flush whatever's left — some upstreams omit the final \n\n.
    if !buf.is_empty() {
        process_event(&buf, flavor, &mut usage);
    }
    usage
}

/// Find the byte index *after* the next event terminator (`\n\n` or
/// `\r\n\r\n`). Returns None when no terminator is present yet.
fn next_event_end(buf: &[u8]) -> Option<usize> {
    // Walk once, watching for two consecutive newlines with optional `\r`.
    let mut i = 0;
    while i + 1 < buf.len() {
        if buf[i] == b'\n' && buf[i + 1] == b'\n' {
            return Some(i + 2);
        }
        if i + 3 < buf.len()
            && buf[i] == b'\r'
            && buf[i + 1] == b'\n'
            && buf[i + 2] == b'\r'
            && buf[i + 3] == b'\n'
        {
            return Some(i + 4);
        }
        i += 1;
    }
    None
}

fn process_event(raw: &[u8], flavor: StreamFlavor, usage: &mut TokenUsage) {
    let text = match std::str::from_utf8(raw) {
        Ok(s) => s,
        Err(_) => return,
    };
    let mut data = String::new();
    let mut event_type = String::new();
    for line in text.split(|c| c == '\n' || c == '\r') {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = strip_field(line, "data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(rest);
        } else if let Some(rest) = strip_field(line, "event:") {
            event_type = rest.trim().to_string();
        }
    }
    if data.is_empty() || data.trim() == "[DONE]" {
        return;
    }
    let json: Value = match serde_json::from_str(&data) {
        Ok(v) => v,
        Err(_) => return,
    };
    fold_usage(&json, flavor, &event_type, usage);
}

fn strip_field<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(prefix)?;
    Some(rest.strip_prefix(' ').unwrap_or(rest))
}

fn fold_usage(value: &Value, flavor: StreamFlavor, event_type: &str, usage: &mut TokenUsage) {
    match flavor {
        StreamFlavor::OpenAi => fold_openai(value, usage),
        StreamFlavor::Anthropic => fold_anthropic(value, event_type, usage),
    }
}

fn fold_openai(v: &Value, usage: &mut TokenUsage) {
    let Some(u) = v.get("usage") else {
        return;
    };
    let prompt = u.get("prompt_tokens").and_then(Value::as_u64).unwrap_or(0) as u32;
    let completion = u
        .get("completion_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    let cached = u
        .get("prompt_tokens_details")
        .and_then(|d| d.get("cached_tokens"))
        .and_then(Value::as_u64)
        .or_else(|| u.get("cached_tokens").and_then(Value::as_u64))
        .unwrap_or(0) as u32;
    usage.input_tokens = prompt.saturating_sub(cached);
    usage.output_tokens = completion;
    usage.cache_read_tokens = cached;
}

fn fold_anthropic(v: &Value, event_type: &str, usage: &mut TokenUsage) {
    // `message_start` carries the prompt count + cache reads; the body of
    // `message_delta` events carries the *running* output count, last one
    // wins. Bedrock-anthropic flattens the envelope and skips `event:`,
    // so we also look at `type` on the JSON itself as a fallback.
    let inferred_type = v
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or(event_type);
    let usage_obj = match inferred_type {
        "message_start" => v.get("message").and_then(|m| m.get("usage")),
        _ => v.get("usage"),
    };
    let Some(u) = usage_obj else {
        return;
    };
    if let Some(it) = u.get("input_tokens").and_then(Value::as_u64) {
        usage.input_tokens = it as u32;
    }
    if let Some(ot) = u.get("output_tokens").and_then(Value::as_u64) {
        usage.output_tokens = ot as u32;
    }
    if let Some(c) = u.get("cache_read_input_tokens").and_then(Value::as_u64) {
        usage.cache_read_tokens = c as u32;
    }
    if let Some(c) = u.get("cache_creation_input_tokens").and_then(Value::as_u64) {
        usage.cache_write_tokens = c as u32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    fn drive(flavor: StreamFlavor, chunks: &[&[u8]]) -> TokenUsage {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (tx, rx) = mpsc::unbounded_channel();
        for c in chunks {
            tx.send(Bytes::copy_from_slice(c)).unwrap();
        }
        drop(tx);
        rt.block_on(reconcile(rx, flavor))
    }

    #[test]
    fn openai_extracts_usage_from_final_chunk() {
        let chunks: &[&[u8]] = &[
            b"data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n",
            b"data: {\"choices\":[{\"delta\":{}}],\"usage\":{\"prompt_tokens\":12,\"completion_tokens\":3,\"prompt_tokens_details\":{\"cached_tokens\":2}}}\n\ndata: [DONE]\n\n",
        ];
        let u = drive(StreamFlavor::OpenAi, chunks);
        assert_eq!(u.input_tokens, 10); // 12 prompt − 2 cached
        assert_eq!(u.output_tokens, 3);
        assert_eq!(u.cache_read_tokens, 2);
    }

    #[test]
    fn openai_returns_default_when_usage_missing() {
        let chunks: &[&[u8]] = &[b"data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n"];
        let u = drive(StreamFlavor::OpenAi, chunks);
        assert_eq!(u, TokenUsage::default());
    }

    #[test]
    fn anthropic_collects_input_then_output() {
        let chunks: &[&[u8]] = &[
            b"event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":42,\"output_tokens\":1,\"cache_read_input_tokens\":5}}}\n\n",
            b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"hi\"}}\n\n",
            b"event: message_delta\ndata: {\"type\":\"message_delta\",\"usage\":{\"output_tokens\":17}}\n\n",
            b"event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
        ];
        let u = drive(StreamFlavor::Anthropic, chunks);
        assert_eq!(u.input_tokens, 42);
        assert_eq!(u.output_tokens, 17);
        assert_eq!(u.cache_read_tokens, 5);
    }

    #[test]
    fn handles_chunk_split_mid_event() {
        // Split one OpenAI event across two channel sends. We must still
        // recover the usage cleanly.
        let chunks: &[&[u8]] = &[
            b"data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}],\"usa",
            b"ge\":{\"prompt_tokens\":3,\"completion_tokens\":4}}\n\n",
        ];
        let u = drive(StreamFlavor::OpenAi, chunks);
        assert_eq!(u.input_tokens, 3);
        assert_eq!(u.output_tokens, 4);
    }

    #[test]
    fn anthropic_works_with_crlf_terminators() {
        let chunks: &[&[u8]] =
            &[b"event: message_start\r\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":7,\"output_tokens\":0}}}\r\n\r\n"];
        let u = drive(StreamFlavor::Anthropic, chunks);
        assert_eq!(u.input_tokens, 7);
    }
}
