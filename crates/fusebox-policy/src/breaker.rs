//! Circuit breaker — the autonomous response that makes Fusebox different
//! from a passive observer.
//!
//! State diagram (reference: `架构.md` §4.3):
//!
//! ```text
//!              budget_exceeded || anomaly
//!              ┌──────────────────────────────┐
//!              │                              ▼
//!       ┌──────────┐    cooldown elapsed   ┌──────┐
//!       │  Closed  │◄──success rate ≥ X───│ Half │
//!       │ (allow)  │                       │ Open │
//!       └──────────┘                       └──┬───┘
//!                                             │ failure
//!                                             ▼
//!                                       ┌─────────┐
//!                                       │  Open   │
//!                                       │ (deny)  │
//!                                       └─────────┘
//! ```

use chrono::{DateTime, Utc};
use fusebox_core::DenyReason;
use serde::Serialize;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BreakerLabel {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum BreakerState {
    Closed,
    Open {
        opened_at: DateTime<Utc>,
        cooldown_until: DateTime<Utc>,
        reason: DenyReason,
    },
    HalfOpen {
        entered_at: DateTime<Utc>,
        trials_taken: u32,
        successes: u32,
    },
}

impl BreakerState {
    pub fn label(&self) -> BreakerLabel {
        match self {
            BreakerState::Closed => BreakerLabel::Closed,
            BreakerState::Open { .. } => BreakerLabel::Open,
            BreakerState::HalfOpen { .. } => BreakerLabel::HalfOpen,
        }
    }
}

/// Transition emitted whenever the breaker changes state — used by the
/// proxy to write to the audit log and by the dashboard to re-render.
#[derive(Debug, Clone, Serialize)]
pub struct BreakerTransition {
    pub from: BreakerLabel,
    pub to: BreakerLabel,
    pub at: DateTime<Utc>,
    pub reason: Option<DenyReason>,
}

#[derive(Debug, Clone)]
pub struct Breaker {
    state: BreakerState,
    cooldown: Duration,
    halfopen_trials: u32,
}

impl Breaker {
    pub fn new(cooldown: Duration, halfopen_trials: u32) -> Self {
        Self {
            state: BreakerState::Closed,
            cooldown,
            halfopen_trials,
        }
    }

    pub fn state(&self) -> &BreakerState {
        &self.state
    }

    pub fn label(&self) -> BreakerLabel {
        self.state.label()
    }

    /// Refresh the state for "time passing" effects (Open → HalfOpen after
    /// cooldown). Call this before every read of `label()` from a hot path.
    pub fn refresh(&mut self, now: DateTime<Utc>) -> Option<BreakerTransition> {
        if let BreakerState::Open { cooldown_until, .. } = &self.state {
            if now >= *cooldown_until {
                let from = self.state.label();
                self.state = BreakerState::HalfOpen {
                    entered_at: now,
                    trials_taken: 0,
                    successes: 0,
                };
                return Some(BreakerTransition {
                    from,
                    to: BreakerLabel::HalfOpen,
                    at: now,
                    reason: None,
                });
            }
        }
        None
    }

    /// Trip the breaker open. Always overrides whatever state we're in.
    pub fn trip(&mut self, reason: DenyReason, now: DateTime<Utc>) -> BreakerTransition {
        let from = self.state.label();
        let cooldown_until = now + chrono::Duration::from_std(self.cooldown).unwrap_or_default();
        self.state = BreakerState::Open {
            opened_at: now,
            cooldown_until,
            reason: reason.clone(),
        };
        BreakerTransition {
            from,
            to: BreakerLabel::Open,
            at: now,
            reason: Some(reason),
        }
    }

    /// Record a successful outcome. May transition HalfOpen → Closed.
    pub fn on_success(&mut self, now: DateTime<Utc>) -> Option<BreakerTransition> {
        if let BreakerState::HalfOpen {
            trials_taken,
            successes,
            ..
        } = &mut self.state
        {
            *trials_taken += 1;
            *successes += 1;
            if *successes >= self.halfopen_trials {
                let from = self.state.label();
                self.state = BreakerState::Closed;
                return Some(BreakerTransition {
                    from,
                    to: BreakerLabel::Closed,
                    at: now,
                    reason: None,
                });
            }
        }
        None
    }

    /// Record a failure (upstream 5xx, anomaly, etc). HalfOpen failures
    /// snap the breaker straight back to Open.
    pub fn on_failure(
        &mut self,
        reason: DenyReason,
        now: DateTime<Utc>,
    ) -> Option<BreakerTransition> {
        if matches!(self.state, BreakerState::HalfOpen { .. }) {
            return Some(self.trip(reason, now));
        }
        None
    }

    /// Manual override — admins can force a reset from the dashboard / CLI.
    pub fn manual_reset(&mut self, now: DateTime<Utc>) -> BreakerTransition {
        let from = self.state.label();
        self.state = BreakerState::Closed;
        BreakerTransition {
            from,
            to: BreakerLabel::Closed,
            at: now,
            reason: None,
        }
    }

    /// True if the breaker should currently deny new traffic. HalfOpen
    /// returns false because we want to *probe* the upstream.
    pub fn is_blocking(&self) -> bool {
        matches!(self.state, BreakerState::Open { .. })
    }

    /// During half-open we cap trial volume so we don't accidentally
    /// reopen the firehose while testing.
    pub fn allow_halfopen_trial(&mut self) -> bool {
        if let BreakerState::HalfOpen { trials_taken, .. } = &mut self.state {
            if *trials_taken < self.halfopen_trials * 2 {
                *trials_taken += 1;
                return true;
            }
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration as StdDuration;

    fn now() -> DateTime<Utc> {
        Utc::now()
    }

    #[test]
    fn closed_to_open_to_halfopen_to_closed() {
        let mut b = Breaker::new(StdDuration::from_millis(10), 2);
        assert_eq!(b.label(), BreakerLabel::Closed);

        let t = b.trip(DenyReason::BudgetExceeded { window: "1m".into() }, now());
        assert_eq!(t.to, BreakerLabel::Open);
        assert!(b.is_blocking());

        std::thread::sleep(StdDuration::from_millis(15));
        let t = b.refresh(now()).expect("should transition to half-open");
        assert_eq!(t.to, BreakerLabel::HalfOpen);
        assert!(!b.is_blocking());

        b.on_success(now());
        let t = b.on_success(now()).expect("should close");
        assert_eq!(t.to, BreakerLabel::Closed);
    }

    #[test]
    fn halfopen_failure_reopens() {
        let mut b = Breaker::new(StdDuration::from_secs(60), 5);
        b.trip(DenyReason::AnomalyDetected, now());
        // Force into half-open.
        b.state = BreakerState::HalfOpen {
            entered_at: now(),
            trials_taken: 0,
            successes: 0,
        };
        let t = b
            .on_failure(DenyReason::AnomalyDetected, now())
            .expect("should reopen");
        assert_eq!(t.to, BreakerLabel::Open);
    }

    #[test]
    fn manual_reset_works_from_open() {
        let mut b = Breaker::new(StdDuration::from_secs(60), 5);
        b.trip(DenyReason::Manual, now());
        let t = b.manual_reset(now());
        assert_eq!(t.to, BreakerLabel::Closed);
        assert!(!b.is_blocking());
    }
}
