//! Decisions emitted by the Policy Engine.

use crate::usage::ModelId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DenyReason {
    BudgetExceeded { window: String },
    BreakerOpen,
    AnomalyDetected,
    RateLimit,
    PerRequestCostTooHigh,
    Manual,
}

impl DenyReason {
    pub fn as_label(&self) -> String {
        match self {
            DenyReason::BudgetExceeded { window } => format!("budget exceeded ({window})"),
            DenyReason::BreakerOpen => "breaker open".to_string(),
            DenyReason::AnomalyDetected => "anomaly detected".to_string(),
            DenyReason::RateLimit => "rate limit".to_string(),
            DenyReason::PerRequestCostTooHigh => "per-request cost too high".to_string(),
            DenyReason::Manual => "manually blocked".to_string(),
        }
    }
}

/// Output of the policy engine for a single request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Deny(DenyReason),
    Downgrade { to: ModelId, reason: DenyReason },
}

impl Decision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Decision::Allow | Decision::Downgrade { .. })
    }

    pub fn is_denied(&self) -> bool {
        matches!(self, Decision::Deny(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allow_and_downgrade_are_allowed() {
        assert!(Decision::Allow.is_allowed());
        assert!(Decision::Downgrade {
            to: ModelId::new("gpt-4o-mini"),
            reason: DenyReason::PerRequestCostTooHigh
        }
        .is_allowed());
        assert!(Decision::Deny(DenyReason::BreakerOpen).is_denied());
    }
}
