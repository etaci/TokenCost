//! Process-local in-memory ledger. Useful as the zero-dependency fallback
//! when storage is misconfigured, and as the backbone of unit tests.

use crate::breaker_event::BreakerEvent;
use crate::event::SpendEvent;
use crate::store::{BreakerEventQuery, LedgerStore, SpendQuery, SpendTotals};
use async_trait::async_trait;
use fusebox_core::{CostUsd, Result};
use parking_lot::RwLock;
use std::sync::Arc;

#[derive(Debug, Default)]
pub struct MemoryLedger {
    events: Arc<RwLock<Vec<SpendEvent>>>,
    breaker_events: Arc<RwLock<Vec<BreakerEvent>>>,
}

impl MemoryLedger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.events.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.read().is_empty()
    }
}

#[async_trait]
impl LedgerStore for MemoryLedger {
    async fn record(&self, event: SpendEvent) -> Result<()> {
        self.events.write().push(event);
        Ok(())
    }

    async fn totals(&self, query: &SpendQuery) -> Result<SpendTotals> {
        let events = self.events.read();
        let mut totals = SpendTotals::default();
        for ev in events.iter() {
            if !matches_query(ev, query) {
                continue;
            }
            totals.cost += ev.cost_usd;
            totals.input_tokens += ev.input_tokens as u64;
            totals.output_tokens += ev.output_tokens as u64;
            totals.events += 1;
        }
        Ok(totals)
    }

    async fn list(&self, query: &SpendQuery) -> Result<Vec<SpendEvent>> {
        let events = self.events.read();
        let mut filtered: Vec<SpendEvent> = events
            .iter()
            .filter(|ev| matches_query(ev, query))
            .cloned()
            .collect();
        filtered.sort_by(|a, b| b.ts.cmp(&a.ts));
        if let Some(limit) = query.limit {
            filtered.truncate(limit as usize);
        }
        Ok(filtered)
    }

    async fn record_breaker(&self, event: BreakerEvent) -> Result<()> {
        self.breaker_events.write().push(event);
        Ok(())
    }

    async fn list_breaker_events(&self, query: &BreakerEventQuery) -> Result<Vec<BreakerEvent>> {
        let evs = self.breaker_events.read();
        let mut filtered: Vec<BreakerEvent> = evs
            .iter()
            .filter(|ev| {
                if let Some(t) = &query.tenant {
                    if &ev.tenant_id != t {
                        return false;
                    }
                }
                ev.ts >= query.since
            })
            .cloned()
            .collect();
        filtered.sort_by(|a, b| b.ts.cmp(&a.ts));
        if let Some(limit) = query.limit {
            filtered.truncate(limit as usize);
        }
        Ok(filtered)
    }

    async fn ping(&self) -> Result<()> {
        Ok(())
    }
}

fn matches_query(ev: &SpendEvent, q: &SpendQuery) -> bool {
    if let Some(tenant) = &q.tenant {
        if &ev.tenant_id != tenant {
            return false;
        }
    }
    if ev.ts < q.since {
        return false;
    }
    if let Some(until) = q.until {
        if ev.ts > until {
            return false;
        }
    }
    true
}

impl MemoryLedger {
    /// Total cost across every event. Test helper.
    pub fn total_cost(&self) -> CostUsd {
        let events = self.events.read();
        let sum: f64 = events.iter().map(|e| e.cost_usd.0).sum();
        CostUsd(sum)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::breaker_event::BreakerTransitionKind;
    use crate::event::SpendStatus;
    use chrono::{Duration, Utc};
    use fusebox_core::{ModelId, Provider, TenantId, TokenUsage};

    fn ev(tenant: &str, cost: f64) -> SpendEvent {
        SpendEvent::now(
            TenantId::from(tenant),
            Provider::OpenAI,
            ModelId::new("gpt-4o-mini"),
            TokenUsage {
                input_tokens: 100,
                output_tokens: 50,
                ..Default::default()
            },
            CostUsd(cost),
            SpendStatus::Completed,
        )
    }

    #[tokio::test]
    async fn records_and_totals() {
        let ledger = MemoryLedger::new();
        ledger.record(ev("a", 0.10)).await.unwrap();
        ledger.record(ev("a", 0.20)).await.unwrap();
        ledger.record(ev("b", 1.00)).await.unwrap();

        let totals = ledger
            .totals(&SpendQuery::for_tenant_since(
                TenantId::from("a"),
                Utc::now() - Duration::hours(1),
            ))
            .await
            .unwrap();
        assert!((totals.cost.0 - 0.30).abs() < 1e-9);
        assert_eq!(totals.events, 2);
    }

    #[tokio::test]
    async fn breaker_events_round_trip() {
        let ledger = MemoryLedger::new();
        ledger
            .record_breaker(BreakerEvent::now(
                TenantId::from("a"),
                BreakerTransitionKind::Trip,
                Some("budget_exceeded(1d)".into()),
            ))
            .await
            .unwrap();
        ledger
            .record_breaker(BreakerEvent::now(
                TenantId::from("b"),
                BreakerTransitionKind::ManualReset,
                Some("manual".into()),
            ))
            .await
            .unwrap();

        let all = ledger
            .list_breaker_events(&BreakerEventQuery::since(Utc::now() - Duration::hours(1)))
            .await
            .unwrap();
        assert_eq!(all.len(), 2);

        let only_a = ledger
            .list_breaker_events(&BreakerEventQuery {
                tenant: Some(TenantId::from("a")),
                since: Utc::now() - Duration::hours(1),
                limit: None,
            })
            .await
            .unwrap();
        assert_eq!(only_a.len(), 1);
        assert_eq!(only_a[0].transition, BreakerTransitionKind::Trip);
    }
}
