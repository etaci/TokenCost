//! `fusebox pricing` — manage the local `pricing/*.yaml` table.
//!
//! Right now the only subcommand is `sync`, which fetches LiteLLM's
//! `model_prices_and_context_window.json` (community-maintained, refreshed
//! within hours of a public price change) and rewrites the per-provider
//! YAML files Fusebox embeds at runtime.
//!
//! Mirrors `scripts/sync-pricing.py` line-for-line so CI either path can
//! flag drift. We keep the script around for environments where Python is
//! easier than installing the Fusebox binary; the CLI is the canonical
//! version for end-users who just want `fusebox pricing sync` to work.

use anyhow::{anyhow, Context, Result};
use clap::{Args, Subcommand};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Where LiteLLM publishes the canonical model price feed. We pin to
/// `main`; if upstream ever breaks the JSON shape we'll bump this URL to
/// a tagged release.
const LITELLM_URL: &str = "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json";

/// (litellm provider key, our pricing/<name>.yaml stem, Fusebox Provider enum)
const PROVIDERS: &[(&str, &str, &str)] = &[
    ("openai", "openai", "openai"),
    ("anthropic", "anthropic", "anthropic"),
    ("gemini", "google", "google"),
    ("vertex_ai-language-models", "google", "google"),
    ("bedrock", "bedrock", "bedrock"),
    ("openrouter", "openrouter", "openrouter"),
];

#[derive(Debug, Subcommand)]
pub enum PricingCommand {
    /// Pull LiteLLM's pricing feed and rewrite `pricing/*.yaml`.
    Sync(SyncArgs),
}

#[derive(Debug, Args)]
pub struct SyncArgs {
    /// Output directory. Defaults to `./pricing` (the embedded location).
    #[arg(long, default_value = "pricing")]
    pub dir: PathBuf,
    /// Override the upstream URL. Useful in tests / offline mirrors.
    #[arg(long)]
    pub source: Option<String>,
    /// Don't write anything; just print what would change. The exit code
    /// is non-zero when changes would have been written, so CI can fail
    /// on drift.
    #[arg(long)]
    pub check: bool,
}

pub async fn run(cmd: PricingCommand) -> Result<()> {
    match cmd {
        PricingCommand::Sync(args) => sync(args).await,
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct LiteLlmEntry {
    #[serde(default)]
    litellm_provider: Option<String>,
    #[serde(default)]
    input_cost_per_token: Option<f64>,
    #[serde(default)]
    output_cost_per_token: Option<f64>,
    #[serde(default)]
    cache_read_input_token_cost: Option<f64>,
    #[serde(default)]
    cache_creation_input_token_cost: Option<f64>,
}

async fn sync(args: SyncArgs) -> Result<()> {
    let url = args.source.unwrap_or_else(|| LITELLM_URL.to_string());
    println!("fetching: {url}");
    let body = reqwest::get(&url)
        .await
        .with_context(|| format!("GET {url}"))?
        .error_for_status()
        .with_context(|| format!("non-2xx from {url}"))?
        .text()
        .await
        .context("read body")?;

    // The upstream file occasionally includes a `sample_spec` placeholder we
    // need to drop before deserialising. We use raw `serde_json::Value` so
    // *any* unknown shape is tolerated — we only care about the four fields
    // we list above.
    let raw: serde_json::Value =
        serde_json::from_str(&body).context("parse LiteLLM JSON")?;
    let obj = raw
        .as_object()
        .ok_or_else(|| anyhow!("LiteLLM payload is not a JSON object"))?;

    println!("got {} model entries", obj.len());

    if !args.dir.exists() {
        std::fs::create_dir_all(&args.dir).with_context(|| {
            format!("create pricing dir {}", args.dir.display())
        })?;
    }

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let mut by_file: BTreeMap<&str, BTreeMap<String, ModelOut>> = BTreeMap::new();

    for (raw_name, value) in obj.iter() {
        if raw_name == "sample_spec" {
            continue;
        }
        let entry: LiteLlmEntry = match serde_json::from_value(value.clone()) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let Some(provider) = entry.litellm_provider.as_deref() else {
            continue;
        };
        let Some((_, file_stem, _)) = PROVIDERS.iter().find(|(p, _, _)| matches_provider(provider, p)) else {
            continue;
        };
        let Some(model) = extract_pricing(&entry) else {
            continue;
        };
        // Strip vendor prefix (e.g. "anthropic/claude-3-5-sonnet" → bare name).
        let name = raw_name.split_once('/').map(|(_, r)| r).unwrap_or(raw_name);
        by_file
            .entry(file_stem)
            .or_default()
            .insert(name.to_string(), model);
    }

    let mut changed = false;
    for (_, file_stem, provider_enum) in PROVIDERS {
        let Some(models) = by_file.get(file_stem) else {
            continue;
        };
        if models.is_empty() {
            continue;
        }
        let out_path = args.dir.join(format!("{file_stem}.yaml"));
        let body = dump_yaml(provider_enum, &today, models);
        let existing = std::fs::read_to_string(&out_path).unwrap_or_default();
        if normalise(&existing) == normalise(&body) {
            println!(
                "  {}.yaml: {} models — unchanged",
                file_stem,
                models.len()
            );
            continue;
        }
        if args.check {
            println!(
                "  {}.yaml: {} models — WOULD change",
                file_stem,
                models.len()
            );
            changed = true;
            continue;
        }
        std::fs::write(&out_path, &body).with_context(|| {
            format!("write {}", out_path.display())
        })?;
        println!("  {}.yaml: wrote {} models", file_stem, models.len());
        changed = true;
    }

    if !changed {
        println!("no pricing changes");
        return Ok(());
    }
    if args.check {
        return Err(anyhow!("pricing drift detected — re-run without --check"));
    }
    Ok(())
}

fn matches_provider(actual: &str, key: &str) -> bool {
    if actual == key {
        return true;
    }
    // Vertex-AI Gemini lives under `vertex_ai-language-models` upstream;
    // we group it under the same `google.yaml`.
    if let Some(prefix) = key.strip_suffix("-language-models") {
        return actual.starts_with(prefix);
    }
    false
}

#[derive(Debug)]
struct ModelOut {
    input_per_1m: f64,
    output_per_1m: f64,
    cache_read_per_1m: Option<f64>,
    cache_write_per_1m: Option<f64>,
}

fn extract_pricing(entry: &LiteLlmEntry) -> Option<ModelOut> {
    let input = entry.input_cost_per_token?;
    let output = entry.output_cost_per_token?;
    Some(ModelOut {
        input_per_1m: to_per_million(input),
        output_per_1m: to_per_million(output),
        cache_read_per_1m: entry.cache_read_input_token_cost.map(to_per_million),
        cache_write_per_1m: entry.cache_creation_input_token_cost.map(to_per_million),
    })
}

fn to_per_million(per_token: f64) -> f64 {
    // LiteLLM stores per-token; we render per-1M to match the public pricing
    // pages and keep YAML readable. Round to 6 dp to avoid scientific
    // notation drift in YAML output.
    (per_token * 1_000_000.0 * 1_000_000.0).round() / 1_000_000.0
}

fn quote_if_needed(s: &str) -> String {
    if s.chars().any(|c| {
        matches!(
            c,
            ':' | '#' | '{' | '}' | '[' | ']' | ',' | '&' | '*' | '!' | '|' | '>' | '"' | '\''
                | '%' | '@' | '`'
        )
    }) {
        format!("\"{}\"", s.replace('"', "\\\""))
    } else {
        s.to_string()
    }
}

fn dump_yaml(provider_enum: &str, last_updated: &str, models: &BTreeMap<String, ModelOut>) -> String {
    let mut out = String::new();
    out.push_str(&format!("provider: {provider_enum}\n"));
    out.push_str(&format!("last_updated: \"{last_updated}\"\n"));
    out.push_str("# Auto-generated by `fusebox pricing sync`. Source: LiteLLM\n");
    out.push_str("# (BerriAI/litellm model_prices_and_context_window.json).\n");
    out.push_str("# Hand-edits are okay but will be overwritten on the next sync.\n");
    out.push_str("models:\n");
    for (name, m) in models {
        out.push_str(&format!("  {}:\n", quote_if_needed(name)));
        out.push_str(&format!("    input_per_1m: {}\n", m.input_per_1m));
        out.push_str(&format!("    output_per_1m: {}\n", m.output_per_1m));
        if let Some(v) = m.cache_read_per_1m {
            out.push_str(&format!("    cache_read_per_1m: {v}\n"));
        }
        if let Some(v) = m.cache_write_per_1m {
            out.push_str(&format!("    cache_write_per_1m: {v}\n"));
        }
    }
    out
}

/// Normalise file contents for the "did anything actually change?" check.
/// We drop `last_updated:` because that line moves on every run.
fn normalise(s: &str) -> String {
    s.lines()
        .filter(|l| !l.trim_start().starts_with("last_updated:"))
        .collect::<Vec<_>>()
        .join("\n")
}

// `Path` is only used by tests below; the runtime functions own `PathBuf`.
#[allow(dead_code)]
fn _ensure_path_used(_p: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_vertex_ai_under_google() {
        assert!(matches_provider("vertex_ai-chat", "vertex_ai-language-models"));
        assert!(matches_provider("gemini", "gemini"));
        assert!(!matches_provider("openai", "anthropic"));
    }

    #[test]
    fn to_per_million_handles_small_values() {
        let v = to_per_million(0.000003); // $3/M tokens
        assert!((v - 3.0).abs() < 1e-9);
    }

    #[test]
    fn dump_yaml_is_stable() {
        let mut models = BTreeMap::new();
        models.insert(
            "gpt-4o-mini".to_string(),
            ModelOut {
                input_per_1m: 0.15,
                output_per_1m: 0.6,
                cache_read_per_1m: Some(0.075),
                cache_write_per_1m: None,
            },
        );
        let body = dump_yaml("openai", "2026-05-01", &models);
        assert!(body.contains("provider: openai"));
        assert!(body.contains("gpt-4o-mini:"));
        assert!(body.contains("input_per_1m: 0.15"));
        assert!(body.contains("cache_read_per_1m: 0.075"));
        assert!(!body.contains("cache_write_per_1m"));
    }

    #[test]
    fn normalise_ignores_timestamp_line() {
        let a = "provider: openai\nlast_updated: \"2026-05-01\"\nmodels: {}\n";
        let b = "provider: openai\nlast_updated: \"2099-12-31\"\nmodels: {}\n";
        assert_eq!(normalise(a), normalise(b));
    }
}
