//! Budget specs and rolling-window definitions.

use crate::usage::CostUsd;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Time window over which a budget is evaluated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BudgetWindow {
    Minute,
    Hour,
    Day,
    Week,
    Month,
}

impl BudgetWindow {
    pub fn as_duration(self) -> Duration {
        match self {
            BudgetWindow::Minute => Duration::from_secs(60),
            BudgetWindow::Hour => Duration::from_secs(60 * 60),
            BudgetWindow::Day => Duration::from_secs(60 * 60 * 24),
            BudgetWindow::Week => Duration::from_secs(60 * 60 * 24 * 7),
            // 30-day month for rolling window arithmetic
            BudgetWindow::Month => Duration::from_secs(60 * 60 * 24 * 30),
        }
    }

    pub fn as_label(self) -> &'static str {
        match self {
            BudgetWindow::Minute => "1m",
            BudgetWindow::Hour => "1h",
            BudgetWindow::Day => "1d",
            BudgetWindow::Week => "1w",
            BudgetWindow::Month => "1mo",
        }
    }
}

/// A single spend budget. Multiple budgets per tenant are allowed; any one
/// being exceeded trips the breaker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Budget {
    pub limit_usd: f64,
    pub window: BudgetWindow,
    /// Optional human-readable label for surfacing in the dashboard.
    #[serde(default)]
    pub label: Option<String>,
}

impl Budget {
    pub fn new(limit_usd: f64, window: BudgetWindow) -> Self {
        Self {
            limit_usd,
            window,
            label: None,
        }
    }

    pub fn limit(&self) -> CostUsd {
        CostUsd(self.limit_usd)
    }

    pub fn is_exceeded(&self, current_spend: CostUsd) -> bool {
        current_spend.0 >= self.limit_usd
    }

    pub fn fraction_used(&self, current_spend: CostUsd) -> f64 {
        if self.limit_usd <= 0.0 {
            return 0.0;
        }
        current_spend.0 / self.limit_usd
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_exceeded() {
        let b = Budget::new(10.0, BudgetWindow::Day);
        assert!(!b.is_exceeded(CostUsd(9.99)));
        assert!(b.is_exceeded(CostUsd(10.0)));
        assert!(b.is_exceeded(CostUsd(10.01)));
    }

    #[test]
    fn fraction_used_handles_zero_limit() {
        let b = Budget::new(0.0, BudgetWindow::Day);
        assert_eq!(b.fraction_used(CostUsd(5.0)), 0.0);
    }
}
