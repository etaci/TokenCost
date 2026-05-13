//! EWMA + 3-sigma anomaly detection.
//!
//! Why this and not something fancier:
//!
//! - **Online**: each observation costs O(1); we never store the full
//!   history. Memory is bounded per tenant.
//! - **No training data**: the detector starts in `Warmup` and only emits
//!   verdicts after it has seen enough samples to estimate variance.
//! - **Tunable but sane defaults**: alpha = 0.2 (half-life ~ 3 samples),
//!   sigma = 3 (≈99.7% one-sided coverage under Gaussian assumption).
//!
//! Each tenant gets one detector. The metric we track is per-request
//! cost; spikes show up as z-scores well above the threshold.

use parking_lot::Mutex;

/// One-sided EWMA detector. Construct with [`EwmaDetector::new`]; feed it
/// values via [`observe`](Self::observe).
#[derive(Debug)]
pub struct EwmaDetector {
    alpha: f64,
    sigma_threshold: f64,
    warmup: u32,
    state: Mutex<EwmaState>,
}

#[derive(Debug, Default)]
struct EwmaState {
    /// Exponentially weighted mean.
    mean: f64,
    /// Exponentially weighted *variance* (not standard deviation).
    var: f64,
    /// Total observations seen — used to gate warmup.
    samples: u32,
}

/// Verdict for one observation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnomalyVerdict {
    /// Detector hasn't seen enough samples to make a decision yet.
    Warmup,
    /// Within sigma_threshold of the running mean.
    Normal { z_score: f64 },
    /// Above sigma_threshold — caller should consider tripping.
    Anomalous { z_score: f64 },
}

impl AnomalyVerdict {
    pub fn is_anomalous(self) -> bool {
        matches!(self, AnomalyVerdict::Anomalous { .. })
    }

    pub fn z_score(self) -> Option<f64> {
        match self {
            AnomalyVerdict::Warmup => None,
            AnomalyVerdict::Normal { z_score } | AnomalyVerdict::Anomalous { z_score } => {
                Some(z_score)
            }
        }
    }
}

impl EwmaDetector {
    /// `alpha`  — smoothing factor in (0, 1]. Higher = more weight on the
    ///            most recent sample. 0.2 is a good default.
    /// `sigma_threshold` — z-score above which we flag anomalous (3.0 is
    ///                     the textbook value).
    /// `warmup` — observations to accept silently before starting to flag.
    pub fn new(alpha: f64, sigma_threshold: f64, warmup: u32) -> Self {
        let alpha = alpha.clamp(1e-3, 1.0);
        Self {
            alpha,
            sigma_threshold: sigma_threshold.max(0.0),
            warmup: warmup.max(2),
            state: Mutex::new(EwmaState::default()),
        }
    }

    /// Sane indie default: alpha=0.2, sigma=3, 8 warmup samples.
    pub fn default_indie() -> Self {
        Self::new(0.2, 3.0, 8)
    }

    /// Submit a value, get the verdict for it. The value contributes to
    /// future means whether it was flagged or not — anomalies are still
    /// part of reality.
    pub fn observe(&self, value: f64) -> AnomalyVerdict {
        let mut s = self.state.lock();
        s.samples = s.samples.saturating_add(1);

        if s.samples == 1 {
            // Bootstrap the mean with the first value.
            s.mean = value;
            s.var = 0.0;
            return AnomalyVerdict::Warmup;
        }

        // Decide *before* folding the new sample in — otherwise a single
        // big spike inflates the mean and then can't possibly look far
        // from it. The pre-update mean/var is the recent baseline; the
        // verdict measures how surprising `value` is relative to that.
        let pre_mean = s.mean;
        let pre_var = s.var;
        let delta = value - pre_mean;

        // Welford-style EWMA update for next time:
        //   mean' = mean + alpha * delta
        //   var'  = (1 - alpha) * (var + alpha * delta^2)
        // (Equivalent to a Bayesian update under a leaky prior.)
        s.mean = pre_mean + self.alpha * delta;
        s.var = (1.0 - self.alpha) * (pre_var + self.alpha * delta * delta);

        if s.samples < self.warmup {
            return AnomalyVerdict::Warmup;
        }

        let std = pre_var.sqrt();
        // Variance can stay at zero when every warmup sample was identical
        // — but the 99th observation of the same value shouldn't flag a
        // 10% deviation as anomalous either. Use a fraction of the running
        // mean as a "noise floor": calls within ~5% of the baseline pass
        // the 3σ test, calls 50× the baseline trip it.
        let effective_std = if std < 1e-12 {
            let floor = pre_mean.abs() * 0.05;
            if floor < 1e-12 {
                return AnomalyVerdict::Normal { z_score: 0.0 };
            }
            floor
        } else {
            std
        };
        let z = (value - pre_mean) / effective_std;
        if z.abs() >= self.sigma_threshold {
            AnomalyVerdict::Anomalous { z_score: z }
        } else {
            AnomalyVerdict::Normal { z_score: z }
        }
    }

    /// Forget all history. Useful when an admin manually closes a breaker
    /// — we don't want stale variance to immediately re-trip.
    pub fn reset(&self) {
        *self.state.lock() = EwmaState::default();
    }

    /// (mean, std, samples) — used by status / dashboard panels.
    pub fn snapshot(&self) -> (f64, f64, u32) {
        let s = self.state.lock();
        (s.mean, s.var.sqrt(), s.samples)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warmup_then_stable_then_spike() {
        let det = EwmaDetector::new(0.2, 3.0, 5);
        // Steady $0.10 calls.
        for _ in 0..10 {
            let _ = det.observe(0.10);
        }
        // A normal-sized call should not trip.
        let v = det.observe(0.11);
        assert!(!v.is_anomalous(), "normal value should not flag: {v:?}");

        // A 50x spike should trip.
        let v = det.observe(5.00);
        assert!(v.is_anomalous(), "spike should flag: {v:?}");
    }

    #[test]
    fn warmup_blocks_early_flags() {
        let det = EwmaDetector::new(0.2, 3.0, 6);
        let v = det.observe(1.0);
        assert!(matches!(v, AnomalyVerdict::Warmup));
        let v = det.observe(1000.0); // Would absolutely be anomalous later.
        assert!(matches!(v, AnomalyVerdict::Warmup));
    }

    #[test]
    fn reset_clears_state() {
        let det = EwmaDetector::default_indie();
        for _ in 0..20 {
            let _ = det.observe(0.10);
        }
        let (mean_before, _, samples_before) = det.snapshot();
        assert!(samples_before > 0);
        assert!(mean_before > 0.0);

        det.reset();
        let (_, _, samples_after) = det.snapshot();
        assert_eq!(samples_after, 0);
    }

    #[test]
    fn zero_variance_does_not_panic() {
        let det = EwmaDetector::new(0.2, 3.0, 3);
        // Same value over and over → variance = 0.
        for _ in 0..10 {
            let v = det.observe(2.0);
            assert!(!v.is_anomalous());
        }
    }
}
