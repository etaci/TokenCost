//! Top-level configuration for a Fusebox instance.
//!
//! Layered loading order (later overrides earlier):
//! 1. compiled-in defaults (this file's `Default` impls)
//! 2. `/etc/fusebox/fusebox.yaml` (system-wide, optional)
//! 3. `./fusebox.yaml` (per-project, optional)
//! 4. `$FUSEBOX_CONFIG` (explicit override path)
//! 5. `FUSEBOX_*` environment variables
//!
//! The full layering machinery lives in the proxy crate (figment); this
//! module just defines the schema and sane defaults.

use crate::budget::{Budget, BudgetWindow};
use crate::error::{FuseboxError, Result};
use crate::usage::Provider;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub proxy: ProxyConfig,
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default)]
    pub policy: PolicyConfig,
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
    #[serde(default)]
    pub pricing: PricingConfig,
    #[serde(default)]
    pub telemetry: TelemetryConfig,
}

impl Default for Config {
    fn default() -> Self {
        let mut providers = HashMap::new();
        providers.insert(
            "openai".to_string(),
            ProviderConfig {
                provider: Provider::OpenAI,
                base_url: "https://api.openai.com".to_string(),
            },
        );
        providers.insert(
            "anthropic".to_string(),
            ProviderConfig {
                provider: Provider::Anthropic,
                base_url: "https://api.anthropic.com".to_string(),
            },
        );
        providers.insert(
            "openrouter".to_string(),
            ProviderConfig {
                provider: Provider::OpenRouter,
                base_url: "https://openrouter.ai/api".to_string(),
            },
        );
        providers.insert(
            "google".to_string(),
            ProviderConfig {
                provider: Provider::Google,
                base_url: "https://generativelanguage.googleapis.com".to_string(),
            },
        );
        providers.insert(
            "bedrock".to_string(),
            ProviderConfig {
                provider: Provider::Bedrock,
                // Default region; operators override via fusebox.yaml in
                // multi-region setups. We don't pre-bake SigV4 here, so
                // requests need a pre-signed body or a sidecar that signs.
                base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".to_string(),
            },
        );

        Self {
            proxy: ProxyConfig::default(),
            storage: StorageConfig::default(),
            policy: PolicyConfig::default(),
            providers,
            pricing: PricingConfig::default(),
            telemetry: TelemetryConfig::default(),
        }
    }
}

impl Config {
    pub fn from_yaml_str(yaml: &str) -> Result<Self> {
        serde_yaml::from_str(yaml).map_err(FuseboxError::from)
    }

    pub fn from_yaml_file(path: &std::path::Path) -> Result<Self> {
        let body = std::fs::read_to_string(path)?;
        Self::from_yaml_str(&body)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub bind: String,
    /// Forwarded request timeout in seconds. LLM streams can be long.
    pub upstream_timeout_secs: u64,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0:8080".to_string(),
            upstream_timeout_secs: 600,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum StorageConfig {
    Sqlite { path: PathBuf },
    Postgres { url: String },
    Memory,
}

impl Default for StorageConfig {
    fn default() -> Self {
        // Indie-friendly: zero-config = SQLite at ~/.fusebox/data.db
        let path = dirs_or_cwd().join(".fusebox").join("data.db");
        StorageConfig::Sqlite { path }
    }
}

fn dirs_or_cwd() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    #[serde(default = "default_provider_kind")]
    pub provider: Provider,
    pub base_url: String,
}

fn default_provider_kind() -> Provider {
    Provider::Unknown
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfig {
    /// Default budget applied to any tenant that has no per-tenant override.
    #[serde(default = "default_budget")]
    pub default_budget: Budget,
    /// Per-tenant overrides, keyed by tenant id.
    #[serde(default)]
    pub tenant_budgets: HashMap<String, Vec<Budget>>,
    /// Cooldown in seconds before a tripped breaker tries half-open.
    #[serde(default = "default_cooldown_secs")]
    pub breaker_cooldown_secs: u64,
    /// Half-open sample size: how many trial requests we let through before
    /// deciding to close again.
    #[serde(default = "default_halfopen_trials")]
    pub halfopen_trials: u32,
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            default_budget: default_budget(),
            tenant_budgets: HashMap::new(),
            breaker_cooldown_secs: default_cooldown_secs(),
            halfopen_trials: default_halfopen_trials(),
        }
    }
}

fn default_budget() -> Budget {
    Budget {
        limit_usd: 50.0,
        window: BudgetWindow::Day,
        label: Some("default".to_string()),
    }
}

fn default_cooldown_secs() -> u64 {
    60
}

fn default_halfopen_trials() -> u32 {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingConfig {
    /// Directory containing `*.yaml` files. Defaults to `./pricing`.
    pub dir: Option<PathBuf>,
}

impl Default for PricingConfig {
    fn default() -> Self {
        Self {
            dir: Some(PathBuf::from("pricing")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    /// JSON logs (true) or pretty (false).
    #[serde(default = "default_json_logs")]
    pub json_logs: bool,
    /// Expose `/metrics` for Prometheus scraping.
    #[serde(default = "default_metrics_enabled")]
    pub metrics_enabled: bool,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            json_logs: default_json_logs(),
            metrics_enabled: default_metrics_enabled(),
        }
    }
}

fn default_json_logs() -> bool {
    false
}

fn default_metrics_enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_loads_with_known_providers() {
        let c = Config::default();
        assert!(c.providers.contains_key("openai"));
        assert!(c.providers.contains_key("anthropic"));
    }

    #[test]
    fn parses_minimal_yaml() {
        let yaml = r#"
proxy:
  bind: 0.0.0.0:9999
  upstream_timeout_secs: 30
"#;
        let c = Config::from_yaml_str(yaml).expect("parse");
        assert_eq!(c.proxy.bind, "0.0.0.0:9999");
    }

    #[test]
    fn sqlite_storage_round_trip() {
        let yaml = r#"
storage:
  type: sqlite
  path: /tmp/fb.db
"#;
        let c = Config::from_yaml_str(yaml).expect("parse");
        match c.storage {
            StorageConfig::Sqlite { path } => assert_eq!(path, PathBuf::from("/tmp/fb.db")),
            other => panic!("expected sqlite, got {other:?}"),
        }
    }
}
