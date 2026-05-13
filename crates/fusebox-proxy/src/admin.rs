//! Admin endpoints — config reload, future janitor triggers.
//!
//! Kept in its own module so the routes file doesn't grow another 200 lines
//! and so SIGHUP on Unix can call the same code path as `POST /v1/admin/reload`.

use anyhow::{anyhow, Result};
use fusebox_core::{Config, PricingTable};
use fusebox_policy::engine::PolicyConfig as EnginePolicyConfig;
use serde::Serialize;
use tracing::info;

use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct ReloadSummary {
    pub config_path: Option<String>,
    pub pricing_models: usize,
    pub providers: usize,
    pub tenant_overrides: usize,
}

/// Re-read the on-disk config + pricing files and swap them into AppState.
/// Atomically replaces:
///   - the proxy's `Config`
///   - the `PricingTable`
///   - the `PolicyEngine`'s budget rules
///
/// Doesn't reset breaker state or runtime budget overrides — operators
/// expect those to persist across a reload.
pub async fn reload(state: &AppState) -> Result<ReloadSummary> {
    let path = state.config_path.read().clone();
    let new_config = match &path {
        Some(p) => Config::from_yaml_file(p)
            .map_err(|e| anyhow!("parse {}: {e}", p.display()))?,
        None => {
            // Reload with no path is meaningless but should not be an error
            // — just return current snapshot so the caller knows nothing
            // changed.
            let cfg = state.config.load();
            return Ok(ReloadSummary {
                config_path: None,
                pricing_models: state.pricing.load().len(),
                providers: cfg.providers.len(),
                tenant_overrides: cfg.policy.tenant_budgets.len(),
            });
        }
    };

    let new_pricing = match &new_config.pricing.dir {
        Some(dir) if dir.exists() => {
            PricingTable::load_dir(dir).map_err(|e| anyhow!(e.to_string()))?
        }
        Some(_) => PricingTable::new(),
        None => PricingTable::new(),
    };

    let new_engine_cfg = EnginePolicyConfig::from_core(&new_config.policy);

    let providers = new_config.providers.len();
    let tenant_overrides = new_config.policy.tenant_budgets.len();
    let pricing_models = new_pricing.len();

    state.policy.replace_config(new_engine_cfg);
    state.pricing.replace(new_pricing);
    state.config.replace(new_config);

    info!(
        "reload complete: pricing_models={} providers={} tenant_overrides={}",
        pricing_models, providers, tenant_overrides
    );

    Ok(ReloadSummary {
        config_path: path.map(|p| p.display().to_string()),
        pricing_models,
        providers,
        tenant_overrides,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fusebox_core::{Budget, BudgetWindow, Config};
    use fusebox_ledger::MemoryLedger;
    use fusebox_policy::engine::{PolicyConfig as EnginePolicyConfig, PolicyEngine};
    use std::collections::HashMap;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn build_state() -> AppState {
        let cfg = Config::default();
        let pricing = PricingTable::new();
        let ledger: fusebox_ledger::SharedLedger = Arc::new(MemoryLedger::new());
        let engine_cfg = EnginePolicyConfig::from_core(&cfg.policy);
        let policy = Arc::new(PolicyEngine::new(engine_cfg, ledger.clone()));
        AppState::new(cfg, pricing, ledger, policy)
    }

    #[tokio::test]
    async fn reload_without_path_returns_current_snapshot() {
        let state = build_state();
        let summary = reload(&state).await.unwrap();
        assert!(summary.config_path.is_none());
        assert!(summary.providers > 0);
    }

    #[tokio::test]
    async fn reload_applies_new_default_budget() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("fusebox.yaml");

        // Bootstrap a custom config on disk.
        let mut cfg = Config::default();
        cfg.policy.default_budget = Budget::new(7.5, BudgetWindow::Day);
        // Use an in-memory ledger so reload doesn't try to open the
        // default sqlite path.
        cfg.storage = fusebox_core::StorageConfig::Memory;
        // Detach pricing so reload doesn't try to read ./pricing.
        cfg.pricing.dir = None;
        std::fs::write(&path, serde_yaml::to_string(&cfg).unwrap()).unwrap();

        let state = build_state();
        state.set_config_path(path.clone());

        let summary = reload(&state).await.unwrap();
        assert_eq!(summary.config_path.unwrap(), path.display().to_string());

        // The engine should now report the swapped budget.
        let budgets = state
            .policy
            .budgets_for_tenant(&fusebox_core::TenantId::default_tenant());
        // tenant_budgets is empty in the bootstrapped config, so the
        // default budget (now 7.5) should be returned.
        assert_eq!(budgets.len(), 1);
        assert!((budgets[0].limit_usd - 7.5).abs() < 1e-9);
    }

    #[tokio::test]
    async fn reload_swaps_tenant_budgets() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("fusebox.yaml");

        let mut cfg = Config::default();
        cfg.policy.tenant_budgets.insert(
            "acme".to_string(),
            vec![Budget::new(99.0, BudgetWindow::Day)],
        );
        cfg.storage = fusebox_core::StorageConfig::Memory;
        cfg.pricing.dir = None;
        std::fs::write(&path, serde_yaml::to_string(&cfg).unwrap()).unwrap();

        let state = build_state();
        state.set_config_path(path);

        reload(&state).await.unwrap();
        let budgets = state
            .policy
            .budgets_for_tenant(&fusebox_core::TenantId::from("acme"));
        assert_eq!(budgets.len(), 1);
        assert!((budgets[0].limit_usd - 99.0).abs() < 1e-9);
    }
}
