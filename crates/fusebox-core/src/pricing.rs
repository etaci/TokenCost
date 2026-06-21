//! Pricing tables. Loaded from `pricing/*.yaml` at startup; can be embedded
//! into the binary via `include_str!` for offline use.
//!
//! Layout mirrors the YAML files under `/pricing` at the repo root. Prices
//! are quoted per **1 million tokens** (LiteLLM convention) and divided at
//! query time.

use crate::error::{FuseboxError, Result};
use crate::usage::{CostUsd, ModelId, Provider, TokenUsage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Pricing for a single model, denominated in USD per 1M tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    #[serde(default)]
    pub provider: Option<Provider>,
    pub input_per_1m: f64,
    pub output_per_1m: f64,
    #[serde(default)]
    pub cache_read_per_1m: f64,
    #[serde(default)]
    pub cache_write_per_1m: f64,
    /// Free-form aliases that should resolve to the same pricing entry.
    /// Lets us absorb naming quirks like `gpt-4o-2024-08-06`.
    #[serde(default)]
    pub aliases: Vec<String>,
}

impl ModelPricing {
    pub fn cost_for(&self, usage: &TokenUsage) -> CostUsd {
        let per = 1_000_000f64;
        let input = (usage.input_tokens as f64 / per) * self.input_per_1m;
        let output = (usage.output_tokens as f64 / per) * self.output_per_1m;
        let cache_r = (usage.cache_read_tokens as f64 / per) * self.cache_read_per_1m;
        let cache_w = (usage.cache_write_tokens as f64 / per) * self.cache_write_per_1m;
        CostUsd(input + output + cache_r + cache_w)
    }
}

/// One YAML file's worth of pricing rows.
#[derive(Debug, Clone, Deserialize)]
pub struct PricingFile {
    pub provider: Provider,
    #[serde(default)]
    pub last_updated: Option<String>,
    pub models: HashMap<String, ModelPricing>,
}

/// Aggregate pricing table. The hot path is `get(model)` so we flatten
/// everything into a single map keyed by model id (and aliases).
#[derive(Debug, Clone, Default)]
pub struct PricingTable {
    by_model: HashMap<String, ModelPricing>,
}

impl PricingTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_files(files: Vec<PricingFile>) -> Self {
        let mut by_model = HashMap::new();
        for file in files {
            let provider = file.provider;
            for (name, mut pricing) in file.models {
                pricing.provider.get_or_insert(provider);
                for alias in &pricing.aliases.clone() {
                    by_model.insert(alias.clone(), pricing.clone());
                }
                by_model.insert(name, pricing);
            }
        }
        Self { by_model }
    }

    pub fn load_dir(dir: &Path) -> Result<Self> {
        let mut files = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let is_yaml = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| matches!(e, "yaml" | "yml"))
                .unwrap_or(false);
            if !is_yaml {
                continue;
            }
            let body = std::fs::read_to_string(&path)?;
            let parsed: PricingFile = serde_yaml::from_str(&body).map_err(|e| {
                FuseboxError::Config(format!("failed to parse {}: {e}", path.display()))
            })?;
            files.push(parsed);
        }
        Ok(Self::from_files(files))
    }

    pub fn from_yaml_str(yaml: &str) -> Result<Self> {
        let file: PricingFile = serde_yaml::from_str(yaml)?;
        Ok(Self::from_files(vec![file]))
    }

    pub fn get(&self, model: &ModelId) -> Option<&ModelPricing> {
        self.by_model.get(model.as_str())
    }

    pub fn cost_for(&self, model: &ModelId, usage: &TokenUsage) -> Result<CostUsd> {
        match self.get(model) {
            Some(p) => Ok(p.cost_for(usage)),
            None => Err(FuseboxError::UnknownModel {
                model: model.to_string(),
            }),
        }
    }

    pub fn len(&self) -> usize {
        self.by_model.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_model.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_yaml() -> &'static str {
        r#"
provider: openai
last_updated: "2026-04-01"
models:
  gpt-4o-mini:
    input_per_1m: 0.15
    output_per_1m: 0.60
  gpt-4o:
    input_per_1m: 2.5
    output_per_1m: 10.0
    aliases: ["gpt-4o-2024-08-06"]
"#
    }

    #[test]
    fn parses_and_costs() {
        let table = PricingTable::from_yaml_str(sample_yaml()).expect("parse");
        let cost = table
            .cost_for(
                &ModelId::new("gpt-4o-mini"),
                &TokenUsage {
                    input_tokens: 1_000_000,
                    output_tokens: 500_000,
                    ..Default::default()
                },
            )
            .unwrap();
        // 0.15 * 1 + 0.60 * 0.5 = 0.45
        assert!((cost.dollars() - 0.45).abs() < 1e-9);
    }

    #[test]
    fn alias_resolves() {
        let table = PricingTable::from_yaml_str(sample_yaml()).expect("parse");
        assert!(table.get(&ModelId::new("gpt-4o-2024-08-06")).is_some());
    }

    #[test]
    fn unknown_model_errors() {
        let table = PricingTable::from_yaml_str(sample_yaml()).expect("parse");
        let err = table
            .cost_for(&ModelId::new("nope"), &TokenUsage::default())
            .unwrap_err();
        assert!(matches!(err, FuseboxError::UnknownModel { .. }));
    }
}
