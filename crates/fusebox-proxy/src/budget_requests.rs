//! Budget-increase request workflow.
//!
//! Wraps the small admin loop behind the MCP `request_budget_increase`
//! tool and the future dashboard's "approve" button. Requests live in
//! memory only — the failure mode of a restart losing pending requests is
//! acceptable for now (they'd just be re-filed by the agent on its next
//! check). Persistence is a follow-up if/when this graduates beyond the
//! single-instance deployment.
//!
//! Flow:
//! 1. Agent (or operator) `POST`s a request describing the desired bump.
//! 2. Operator `POST`s to `/approve` → we install a runtime override on
//!    the policy engine so the new limit kicks in *immediately*.
//! 3. The request transitions to `Approved`; subsequent listings show it.
//!
//! Approval is *only* a runtime override. It does **not** rewrite
//! `fusebox.yaml`, so a restart returns to the configured budget — which
//! is the desired behaviour for "temporary" bumps.

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use dashmap::DashMap;
use fusebox_core::{Budget, BudgetWindow, TenantId};
use fusebox_policy::SharedPolicy;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestStatus {
    Pending,
    Approved,
    Rejected,
    Expired,
}

impl RequestStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            RequestStatus::Pending => "pending",
            RequestStatus::Approved => "approved",
            RequestStatus::Rejected => "rejected",
            RequestStatus::Expired => "expired",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetRequest {
    pub id: Uuid,
    pub tenant: TenantId,
    pub window: BudgetWindow,
    pub requested_limit_usd: f64,
    pub reason: Option<String>,
    pub requested_at: DateTime<Utc>,
    pub status: RequestStatus,
    pub decided_at: Option<DateTime<Utc>>,
    pub decided_by: Option<String>,
    pub decision_note: Option<String>,
    /// When approved, the override expires after this many seconds.
    /// `None` → no expiry (until manually cleared / process restart).
    pub ttl_seconds: Option<u64>,
}

impl BudgetRequest {
    pub fn new(
        tenant: TenantId,
        window: BudgetWindow,
        requested_limit_usd: f64,
        reason: Option<String>,
        ttl_seconds: Option<u64>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            tenant,
            window,
            requested_limit_usd,
            reason,
            requested_at: Utc::now(),
            status: RequestStatus::Pending,
            decided_at: None,
            decided_by: None,
            decision_note: None,
            ttl_seconds,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BudgetRequestError {
    #[error("budget request not found: {0}")]
    NotFound(Uuid),
    #[error("budget request is not pending (current status: {0})")]
    NotPending(&'static str),
    #[error("invalid request: {0}")]
    Invalid(String),
}

/// In-memory store, cheap to clone (Arc inside).
#[derive(Debug, Clone)]
pub struct BudgetRequestStore {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    requests: DashMap<Uuid, BudgetRequest>,
    /// Atomic counter so we can sort listings stably even when many
    /// requests share a sub-millisecond timestamp.
    seq: AtomicU64,
}

impl Default for BudgetRequestStore {
    fn default() -> Self {
        Self::new()
    }
}

impl BudgetRequestStore {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                requests: DashMap::new(),
                seq: AtomicU64::new(0),
            }),
        }
    }

    pub fn create(&self, mut req: BudgetRequest) -> Result<BudgetRequest, BudgetRequestError> {
        if req.requested_limit_usd <= 0.0 {
            return Err(BudgetRequestError::Invalid(
                "requested_limit_usd must be positive".into(),
            ));
        }
        // Stable monotonic tiebreaker for sort.
        let _ = self.inner.seq.fetch_add(1, Ordering::Relaxed);
        req.status = RequestStatus::Pending;
        self.inner.requests.insert(req.id, req.clone());
        Ok(req)
    }

    pub fn get(&self, id: Uuid) -> Option<BudgetRequest> {
        self.inner.requests.get(&id).map(|r| r.clone())
    }

    /// Returns a snapshot. Optionally filter by status; pass `None` to get
    /// every request. The order is newest-first by `requested_at`.
    pub fn list(&self, status: Option<RequestStatus>) -> Vec<BudgetRequest> {
        let mut out: Vec<BudgetRequest> = self
            .inner
            .requests
            .iter()
            .filter_map(|r| {
                let v = r.value();
                if let Some(s) = status {
                    if v.status != s {
                        return None;
                    }
                }
                Some(v.clone())
            })
            .collect();
        out.sort_by(|a, b| b.requested_at.cmp(&a.requested_at));
        out
    }

    /// Approve a pending request. Installs the runtime override on the
    /// policy engine so the new limit takes effect immediately.
    pub fn approve(
        &self,
        id: Uuid,
        approver: Option<String>,
        note: Option<String>,
        policy: &SharedPolicy,
    ) -> Result<BudgetRequest, BudgetRequestError> {
        let mut entry = self
            .inner
            .requests
            .get_mut(&id)
            .ok_or(BudgetRequestError::NotFound(id))?;
        if entry.status != RequestStatus::Pending {
            return Err(BudgetRequestError::NotPending(entry.status.as_str()));
        }
        entry.status = RequestStatus::Approved;
        entry.decided_at = Some(Utc::now());
        entry.decided_by = approver;
        entry.decision_note = note;
        let snapshot = entry.clone();
        drop(entry);
        // Install override.
        let budget = Budget {
            limit_usd: snapshot.requested_limit_usd,
            window: snapshot.window,
            label: Some(format!("approved-{}", short_id(snapshot.id))),
        };
        policy.set_runtime_budget(snapshot.tenant.clone(), vec![budget]);

        // Schedule expiry if a TTL was set.
        if let Some(ttl) = snapshot.ttl_seconds {
            let tenant = snapshot.tenant.clone();
            let req_id = snapshot.id;
            let policy_clone = policy.clone();
            let store = self.clone();
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(ttl)).await;
                policy_clone.clear_runtime_budget(&tenant);
                if let Some(mut r) = store.inner.requests.get_mut(&req_id) {
                    if r.status == RequestStatus::Approved {
                        r.status = RequestStatus::Expired;
                    }
                }
            });
        }
        Ok(snapshot)
    }

    pub fn reject(
        &self,
        id: Uuid,
        rejector: Option<String>,
        note: Option<String>,
    ) -> Result<BudgetRequest, BudgetRequestError> {
        let mut entry = self
            .inner
            .requests
            .get_mut(&id)
            .ok_or(BudgetRequestError::NotFound(id))?;
        if entry.status != RequestStatus::Pending {
            return Err(BudgetRequestError::NotPending(entry.status.as_str()));
        }
        entry.status = RequestStatus::Rejected;
        entry.decided_at = Some(Utc::now());
        entry.decided_by = rejector;
        entry.decision_note = note;
        Ok(entry.clone())
    }

    /// Janitor — drop entries older than `retention` regardless of status.
    /// Called periodically by the proxy.
    pub fn purge_older_than(&self, retention: ChronoDuration) -> usize {
        let cutoff = Utc::now() - retention;
        let to_remove: Vec<Uuid> = self
            .inner
            .requests
            .iter()
            .filter(|r| r.requested_at < cutoff)
            .map(|r| *r.key())
            .collect();
        for id in &to_remove {
            self.inner.requests.remove(id);
        }
        to_remove.len()
    }
}

fn short_id(id: Uuid) -> String {
    id.to_string().chars().take(8).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fusebox_ledger::MemoryLedger;
    use fusebox_policy::engine::{PolicyConfig as EnginePolicyConfig, PolicyEngine};
    use std::collections::HashMap;
    use std::sync::Arc;

    fn make_policy() -> SharedPolicy {
        let cfg = EnginePolicyConfig {
            default_budgets: vec![Budget::new(10.0, BudgetWindow::Day)],
            tenant_budgets: HashMap::new(),
            breaker_cooldown: std::time::Duration::from_secs(60),
            halfopen_trials: 3,
            anomaly_alpha: 0.2,
            anomaly_sigma: 3.0,
            anomaly_warmup: 8,
            anomaly_enabled: false,
        };
        let ledger = Arc::new(MemoryLedger::new());
        Arc::new(PolicyEngine::new(cfg, ledger))
    }

    #[test]
    fn create_and_list() {
        let store = BudgetRequestStore::new();
        let req = BudgetRequest::new(
            TenantId::from("acme"),
            BudgetWindow::Day,
            50.0,
            Some("for batch backfill".into()),
            None,
        );
        let created = store.create(req).unwrap();
        let listed = store.list(Some(RequestStatus::Pending));
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.id);
    }

    #[test]
    fn approve_installs_runtime_override() {
        let store = BudgetRequestStore::new();
        let policy = make_policy();
        let tenant = TenantId::from("acme");
        let req = BudgetRequest::new(tenant.clone(), BudgetWindow::Day, 100.0, None, None);
        let created = store.create(req).unwrap();
        store
            .approve(created.id, Some("alice".into()), None, &policy)
            .unwrap();
        let budgets = policy.budgets_for_tenant(&tenant);
        assert_eq!(budgets.len(), 1);
        assert!((budgets[0].limit_usd - 100.0).abs() < 1e-9);
    }

    #[test]
    fn cannot_double_approve() {
        let store = BudgetRequestStore::new();
        let policy = make_policy();
        let tenant = TenantId::from("acme");
        let req = BudgetRequest::new(tenant, BudgetWindow::Day, 100.0, None, None);
        let created = store.create(req).unwrap();
        store.approve(created.id, None, None, &policy).unwrap();
        let err = store.approve(created.id, None, None, &policy).unwrap_err();
        assert!(matches!(err, BudgetRequestError::NotPending(_)));
    }

    #[test]
    fn reject_keeps_existing_budget() {
        let store = BudgetRequestStore::new();
        let policy = make_policy();
        let tenant = TenantId::from("acme");
        let req = BudgetRequest::new(tenant.clone(), BudgetWindow::Day, 100.0, None, None);
        let created = store.create(req).unwrap();
        store
            .reject(created.id, Some("alice".into()), Some("no budget".into()))
            .unwrap();
        let budgets = policy.budgets_for_tenant(&tenant);
        // Default (10.0) remains since no override.
        assert!((budgets[0].limit_usd - 10.0).abs() < 1e-9);
    }
}
