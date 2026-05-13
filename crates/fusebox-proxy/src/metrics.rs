//! Prometheus exporter setup. Mounted at `GET /metrics`.

use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use once_cell::sync::OnceCell;

static HANDLE: OnceCell<PrometheusHandle> = OnceCell::new();

/// Install the global recorder. Idempotent — repeated calls are a no-op.
/// Returns a handle whose `render()` produces the exposition format.
pub fn install() -> &'static PrometheusHandle {
    HANDLE.get_or_init(|| {
        PrometheusBuilder::new()
            .install_recorder()
            .expect("install prometheus recorder")
    })
}

pub fn render() -> String {
    install().render()
}

/// Counter names we use across the proxy. Keeping them centralised here
/// keeps cardinality drift out of the hot path.
pub mod names {
    pub const REQUESTS_TOTAL: &str = "fusebox_requests_total";
    pub const DENIED_TOTAL: &str = "fusebox_requests_denied_total";
    pub const UPSTREAM_FAILED_TOTAL: &str = "fusebox_upstream_failed_total";
    pub const COST_USD: &str = "fusebox_cost_usd_total";
    pub const TOKENS_TOTAL: &str = "fusebox_tokens_total";
}
