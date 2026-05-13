//! Per-tenant policy engine. Wires the ledger, the budget config, and the
//! circuit breaker together into a single `evaluate` call.

use crate::anomaly::{AnomalyVerdict, EwmaDetector};
use crate::breaker::{Breaker, BreakerLabel, BreakerState, BreakerTransition};
use crate::estimate::RequestEstimate;
use chrono::Utc;
use dashmap::DashMap;
use fusebox_core::{Budget, BudgetWindow, CostUsd, Decision, DenyReason, Result, TenantId};
use fusebox_ledger::{
    BreakerEvent, BreakerTransitionKind, SharedLedger, SpendQuery,
};
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

/// Static configuration the engine needs at construction time.
#[derive(Debug, Clone)]
pub struct PolicyConfig {
    pub default_budgets: Vec<Budget>,
    pub tenant_budgets: std::collections::HashMap<TenantId, Vec<Budget>>,
    pub breaker_cooldown: Duration,
    pub halfopen_trials: u32,
    /// EWMA smoothing factor (0, 1]. 0.2 ≈ half-life of 3 samples.
    pub anomaly_alpha: f64,
    /// Z-score threshold above which the breaker trips.
    pub anomaly_sigma: f64,
    /// Observations to accept silently before flagging anything.
    pub anomaly_warmup: u32,
    /// Master switch for the EWMA detector.
    pub anomaly_enabled: bool,
}

impl PolicyConfig {
    pub fn from_core(cfg: &fusebox_core::config::PolicyConfig) -> Self {
        let tenant_budgets = cfg
            .tenant_budgets
            .iter()
            .map(|(k, v)| (TenantId::from(k.as_str()), v.clone()))
            .collect();
        Self {
            default_budgets: vec![cfg.default_budget.clone()],
            tenant_budgets,
            breaker_cooldown: Duration::from_secs(cfg.breaker_cooldown_secs),
            halfopen_trials: cfg.halfopen_trials,
            anomaly_alpha: 0.2,
            anomaly_sigma: 3.0,
            anomaly_warmup: 8,
            anomaly_enabled: true,
        }
    }
}

pub type SharedPolicy = Arc<PolicyEngine>;

#[derive(Debug)]
pub struct PolicyEngine {
    /// Behind a lock so `replace_config` can swap budget rules on SIGHUP /
    /// admin reload without a process restart. Read-only fast path is a
    /// quick clone of `Arc<PolicyConfig>` (see `current_config`).
    config: RwLock<Arc<PolicyConfig>>,
    ledger: SharedLedger,
    breakers: DashMap<TenantId, parking_lot::Mutex<Breaker>>,
    detectors: DashMap<TenantId, EwmaDetector>,
    /// Per-tenant overrides applied at runtime (e.g. approved budget-increase
    /// requests, dashboard tweaks). Take precedence over `config.tenant_budgets`
    /// and the global default. Cleared per-tenant via `clear_runtime_budget`.
    runtime_budgets: DashMap<TenantId, Vec<Budget>>,
}

impl PolicyEngine {
    pub fn new(config: PolicyConfig, ledger: SharedLedger) -> Self {
        Self {
            config: RwLock::new(Arc::new(config)),
            ledger,
            breakers: DashMap::new(),
            detectors: DashMap::new(),
            runtime_budgets: DashMap::new(),
        }
    }

    /// Snapshot the current config. Used by hot-path readers — they get a
    /// cheap `Arc` clone, so a swap concurrent with their read is safe.
    fn current_config(&self) -> Arc<PolicyConfig> {
        self.config.read().clone()
    }

    /// Replace the budget configuration. Called by the proxy when the
    /// admin reload endpoint fires (or SIGHUP on Unix). Cooldown /
    /// halfopen / anomaly tunables also update, but existing breakers
    /// keep their *current* state — flipping them to closed silently
    /// would discard real protection.
    pub fn replace_config(&self, new_config: PolicyConfig) {
        *self.config.write() = Arc::new(new_config);
    }

    /// Get-or-create the breaker for a tenant.
    fn breaker_for(
        &self,
        tenant: &TenantId,
    ) -> dashmap::mapref::one::Ref<'_, TenantId, parking_lot::Mutex<Breaker>> {
        // Fast path: already present.
        if let Some(r) = self.breakers.get(tenant) {
            return r;
        }
        // Slow path: atomic insert via entry API. or_insert_with is safe
        // under contention; if two tasks race, only one closure runs.
        let cfg = self.current_config();
        self.breakers
            .entry(tenant.clone())
            .or_insert_with(|| {
                parking_lot::Mutex::new(Breaker::new(
                    cfg.breaker_cooldown,
                    cfg.halfopen_trials,
                ))
            });
        self.breakers
            .get(tenant)
            .expect("just inserted via entry API")
    }

    fn detector_for(
        &self,
        tenant: &TenantId,
    ) -> dashmap::mapref::one::Ref<'_, TenantId, EwmaDetector> {
        if let Some(r) = self.detectors.get(tenant) {
            return r;
        }
        let cfg = self.current_config();
        self.detectors.entry(tenant.clone()).or_insert_with(|| {
            EwmaDetector::new(
                cfg.anomaly_alpha,
                cfg.anomaly_sigma,
                cfg.anomaly_warmup,
            )
        });
        self.detectors.get(tenant).expect("just inserted")
    }

    pub fn breaker_label(&self, tenant: &TenantId) -> BreakerLabel {
        let entry = self.breaker_for(tenant);
        let mut b = entry.lock();
        b.refresh(Utc::now());
        b.label()
    }

    pub fn breaker_state(&self, tenant: &TenantId) -> BreakerState {
        let entry = self.breaker_for(tenant);
        let mut b = entry.lock();
        b.refresh(Utc::now());
        b.state().clone()
    }

    /// Snapshot of every tenant whose breaker has ever been touched. The
    /// dashboard polls this for the breaker grid; the CLI uses it for
    /// `fusebox status --all`.
    pub fn breaker_snapshot(&self) -> Vec<(TenantId, BreakerState)> {
        let now = Utc::now();
        let mut out = Vec::with_capacity(self.breakers.len());
        for entry in self.breakers.iter() {
            let mut b = entry.value().lock();
            b.refresh(now);
            out.push((entry.key().clone(), b.state().clone()));
        }
        out
    }

    fn budgets_for(&self, tenant: &TenantId) -> Vec<Budget> {
        if let Some(over) = self.runtime_budgets.get(tenant) {
            return over.value().clone();
        }
        let cfg = self.current_config();
        cfg.tenant_budgets
            .get(tenant)
            .cloned()
            .unwrap_or_else(|| cfg.default_budgets.clone())
    }

    /// Install a runtime budget override (e.g. an approved budget-increase
    /// request). Takes effect on the very next `evaluate` call without a
    /// restart. Pass an empty vec to disable budget gating for the tenant.
    pub fn set_runtime_budget(&self, tenant: TenantId, budgets: Vec<Budget>) {
        self.runtime_budgets.insert(tenant, budgets);
    }

    /// Drop the runtime override, falling back to config / default.
    pub fn clear_runtime_budget(&self, tenant: &TenantId) -> bool {
        self.runtime_budgets.remove(tenant).is_some()
    }

    /// Public read for the spend / budget panel.
    pub fn budgets_for_tenant(&self, tenant: &TenantId) -> Vec<Budget> {
        self.budgets_for(tenant)
    }

    /// Pre-flight check. Returns `Decision::Deny` if we know we can't
    /// safely forward. Returns `Decision::Allow` if all budgets are fine
    /// even when the estimated cost lands.
    pub async fn evaluate(
        &self,
        tenant: &TenantId,
        estimate: &RequestEstimate,
    ) -> Result<Decision> {
        // 1. Honor breaker state first — cheapest check.
        {
            let entry = self.breaker_for(tenant);
            let mut b = entry.lock();
            b.refresh(Utc::now());
            if b.is_blocking() {
                return Ok(Decision::Deny(DenyReason::BreakerOpen));
            }
            // Half-open trial gating: cap probe traffic.
            if !b.allow_halfopen_trial() {
                return Ok(Decision::Deny(DenyReason::BreakerOpen));
            }
        }

        // 2. Walk every applicable budget. Any fail = deny.
        let budgets = self.budgets_for(tenant);
        for budget in &budgets {
            let projected = self
                .projected_spend(tenant, budget.window, estimate.estimated_cost)
                .await?;
            if budget.is_exceeded(projected) {
                return Ok(Decision::Deny(DenyReason::BudgetExceeded {
                    window: budget.window.as_label().to_string(),
                }));
            }
        }

        Ok(Decision::Allow)
    }

    /// Total spend over the window plus a hypothetical incremental cost.
    /// Used by `evaluate` to ask "if we forward this request, will we
    /// blow past the budget?"
    async fn projected_spend(
        &self,
        tenant: &TenantId,
        window: BudgetWindow,
        increment: CostUsd,
    ) -> Result<CostUsd> {
        let totals = self
            .ledger
            .totals(&SpendQuery::for_tenant_since(
                tenant.clone(),
                Utc::now() - chrono::Duration::from_std(window.as_duration()).unwrap_or_default(),
            ))
            .await?;
        Ok(totals.cost + increment)
    }

    /// Expose current spend for a tenant + window, no projection. Used by
    /// dashboard and `/v1/spend`.
    pub async fn spend_for(
        &self,
        tenant: &TenantId,
        window: BudgetWindow,
    ) -> Result<CostUsd> {
        let totals = self
            .ledger
            .totals(&SpendQuery::for_tenant_since(
                tenant.clone(),
                Utc::now() - chrono::Duration::from_std(window.as_duration()).unwrap_or_default(),
            ))
            .await?;
        Ok(totals.cost)
    }

    /// Post-flight reconciliation. Called after upstream returns the actual
    /// usage. Trips the breaker if the budget is now exceeded with real
    /// numbers, in which case the *next* call gets denied.
    pub async fn record_outcome(
        &self,
        tenant: &TenantId,
        actual_cost: CostUsd,
        succeeded: bool,
    ) -> Result<Option<BreakerTransition>> {
        let now = Utc::now();

        if !succeeded {
            // Half-open failures snap back to Open.
            let trans = {
                let entry = self.breaker_for(tenant);
                let mut b = entry.lock();
                b.on_failure(DenyReason::AnomalyDetected, now)
            };
            if let Some(t) = &trans {
                self.audit_transition(tenant, t, Some("upstream_failure")).await;
            }
            return Ok(trans);
        }

        // Successful outcome: maybe close a half-open breaker.
        let success_transition = {
            let entry = self.breaker_for(tenant);
            let mut b = entry.lock();
            b.on_success(now)
        };
        if let Some(t) = &success_transition {
            self.audit_transition(tenant, t, Some("halfopen_recovered")).await;
        }

        // Anomaly detection on realised cost. We feed *actual* cost (not
        // estimate) because the estimate's variance is a lower bound only.
        if self.current_config().anomaly_enabled && actual_cost.dollars() > 0.0 {
            let verdict = {
                let entry = self.detector_for(tenant);
                entry.observe(actual_cost.dollars())
            };
            if let AnomalyVerdict::Anomalous { z_score } = verdict {
                let entry = self.breaker_for(tenant);
                let mut b = entry.lock();
                let trip = b.trip(DenyReason::AnomalyDetected, now);
                warn!(
                    tenant = %tenant,
                    z = z_score,
                    cost = actual_cost.dollars(),
                    "breaker tripped — spend anomaly"
                );
                drop(b);
                drop(entry);
                let reason = format!("anomaly(z={:.2})", z_score);
                self.audit_transition(tenant, &trip, Some(&reason)).await;
                return Ok(Some(trip));
            }
        }

        // Budget post-check: if we crossed any limit with real numbers,
        // trip the breaker right now so the next request is rejected.
        let budgets = self.budgets_for(tenant);
        for budget in &budgets {
            let totals = self
                .ledger
                .totals(&SpendQuery::for_tenant_since(
                    tenant.clone(),
                    now - chrono::Duration::from_std(budget.window.as_duration())
                        .unwrap_or_default(),
                ))
                .await?;
            if budget.is_exceeded(totals.cost + actual_cost) {
                let trip = {
                    let entry = self.breaker_for(tenant);
                    let mut b = entry.lock();
                    b.trip(
                        DenyReason::BudgetExceeded {
                            window: budget.window.as_label().to_string(),
                        },
                        now,
                    )
                };
                warn!(
                    tenant = %tenant,
                    window = budget.window.as_label(),
                    spent = totals.cost.0 + actual_cost.0,
                    limit = budget.limit_usd,
                    "breaker tripped — budget exceeded"
                );
                let reason = format!("budget_exceeded({})", budget.window.as_label());
                self.audit_transition(tenant, &trip, Some(&reason)).await;
                return Ok(Some(trip));
            }
        }

        if let Some(t) = &success_transition {
            info!(tenant = %tenant, "breaker {:?} → {:?}", t.from, t.to);
        }
        Ok(success_transition)
    }

    pub fn manual_reset(&self, tenant: &TenantId) -> BreakerTransition {
        let trans = {
            let entry = self.breaker_for(tenant);
            let mut b = entry.lock();
            b.manual_reset(Utc::now())
        };
        // Forget anomaly history so we don't immediately re-trip.
        if let Some(d) = self.detectors.get(tenant) {
            d.reset();
        }
        trans
    }

    /// Async wrapper over `manual_reset` that also writes the audit row.
    pub async fn manual_reset_audited(&self, tenant: &TenantId) -> BreakerTransition {
        let t = self.manual_reset(tenant);
        self.audit_transition(tenant, &t, Some("manual")).await;
        t
    }

    async fn audit_transition(
        &self,
        tenant: &TenantId,
        trans: &BreakerTransition,
        reason: Option<&str>,
    ) {
        let kind = match trans.to {
            BreakerLabel::Open => BreakerTransitionKind::Trip,
            BreakerLabel::HalfOpen => BreakerTransitionKind::HalfOpen,
            BreakerLabel::Closed => {
                if matches!(reason, Some("manual")) {
                    BreakerTransitionKind::ManualReset
                } else {
                    BreakerTransitionKind::Close
                }
            }
        };
        let event = BreakerEvent::now(tenant.clone(), kind, reason.map(|r| r.to_string()));
        if let Err(e) = self.ledger.record_breaker(event).await {
            warn!("audit_transition failed: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fusebox_core::{ModelId, Provider, TokenUsage};
    use fusebox_ledger::{LedgerStore, MemoryLedger, SpendEvent};
    use std::collections::HashMap;
    use std::sync::Arc;

    fn engine_with_budget(limit: f64, window: BudgetWindow) -> (Arc<MemoryLedger>, PolicyEngine) {
        let ledger = Arc::new(MemoryLedger::new());
        let cfg = PolicyConfig {
            default_budgets: vec![Budget::new(limit, window)],
            tenant_budgets: HashMap::new(),
            breaker_cooldown: Duration::from_millis(50),
            halfopen_trials: 2,
            anomaly_alpha: 0.2,
            anomaly_sigma: 3.0,
            anomaly_warmup: 8,
            anomaly_enabled: true,
        };
        let engine = PolicyEngine::new(cfg, ledger.clone());
        (ledger, engine)
    }

    fn engine_with_anomaly(enabled: bool) -> (Arc<MemoryLedger>, PolicyEngine) {
        let ledger = Arc::new(MemoryLedger::new());
        let cfg = PolicyConfig {
            default_budgets: vec![Budget::new(10_000.0, BudgetWindow::Day)], // never trips on budget
            tenant_budgets: HashMap::new(),
            breaker_cooldown: Duration::from_millis(50),
            halfopen_trials: 2,
            anomaly_alpha: 0.5,
            anomaly_sigma: 3.0,
            anomaly_warmup: 4,
            anomaly_enabled: enabled,
        };
        let engine = PolicyEngine::new(cfg, ledger.clone());
        (ledger, engine)
    }

    fn estimate(cost: f64) -> RequestEstimate {
        RequestEstimate::new(
            Provider::OpenAI,
            ModelId::new("gpt-4o-mini"),
            TokenUsage::default(),
            CostUsd(cost),
        )
    }

    #[tokio::test]
    async fn allows_when_budget_has_room() {
        let (_l, engine) = engine_with_budget(10.0, BudgetWindow::Day);
        let tenant = TenantId::from("t1");
        let d = engine.evaluate(&tenant, &estimate(0.05)).await.unwrap();
        assert_eq!(d, Decision::Allow);
    }

    #[tokio::test]
    async fn denies_when_estimate_blows_budget() {
        let (_l, engine) = engine_with_budget(0.10, BudgetWindow::Day);
        let tenant = TenantId::from("t1");
        let d = engine.evaluate(&tenant, &estimate(0.50)).await.unwrap();
        assert!(matches!(d, Decision::Deny(_)));
    }

    #[tokio::test]
    async fn trips_breaker_when_actual_exceeds() {
        let (ledger, engine) = engine_with_budget(0.20, BudgetWindow::Day);
        let tenant = TenantId::from("t1");
        // Allow tiny request through, then have actual cost blow past limit.
        ledger
            .record(SpendEvent::now(
                tenant.clone(),
                Provider::OpenAI,
                ModelId::new("gpt-4o-mini"),
                TokenUsage::default(),
                CostUsd(0.10),
                fusebox_ledger::event::SpendStatus::Completed,
            ))
            .await
            .unwrap();

        // Record post-flight outcome that crosses the threshold.
        let t = engine
            .record_outcome(&tenant, CostUsd(0.15), true)
            .await
            .unwrap();
        assert!(t.is_some(), "breaker should have tripped");

        // Next evaluation must be denied.
        let d = engine.evaluate(&tenant, &estimate(0.001)).await.unwrap();
        assert!(matches!(d, Decision::Deny(DenyReason::BreakerOpen)));
    }

    #[tokio::test]
    async fn anomaly_detector_trips_on_spike() {
        let (_l, engine) = engine_with_anomaly(true);
        let tenant = TenantId::from("indie");
        // Feed steady $0.01 calls so warmup completes with low variance.
        for _ in 0..20 {
            let _ = engine.record_outcome(&tenant, CostUsd(0.01), true).await.unwrap();
        }
        // Sudden 100x spike.
        let t = engine.record_outcome(&tenant, CostUsd(1.0), true).await.unwrap();
        assert!(t.is_some(), "anomaly should have tripped breaker");
        let d = engine.evaluate(&tenant, &estimate(0.001)).await.unwrap();
        assert!(matches!(d, Decision::Deny(DenyReason::BreakerOpen)));
    }

    #[tokio::test]
    async fn manual_reset_clears_anomaly_state() {
        let (_l, engine) = engine_with_anomaly(true);
        let tenant = TenantId::from("indie");
        for _ in 0..20 {
            let _ = engine.record_outcome(&tenant, CostUsd(0.01), true).await.unwrap();
        }
        let _ = engine.record_outcome(&tenant, CostUsd(1.0), true).await.unwrap();
        engine.manual_reset_audited(&tenant).await;
        // After reset, a single small call should not panic / re-trip.
        let t = engine.record_outcome(&tenant, CostUsd(0.01), true).await.unwrap();
        // Could be None (closed → closed); critically must not be Trip.
        if let Some(trans) = t {
            assert_ne!(trans.to, BreakerLabel::Open);
        }
    }
}
