//! Tenant identification.
//!
//! The proxy never stores upstream API keys. Tenants are identified by an
//! `X-Fusebox-Tenant` header (or its lowercase `x-fusebox-tenant` form);
//! when missing we fall back to the singleton `default` tenant so the
//! single-user / indie flow "just works" without configuration.

use axum::http::HeaderMap;
use fusebox_core::TenantId;

pub const TENANT_HEADER: &str = "x-fusebox-tenant";
pub const PROJECT_HEADER: &str = "x-fusebox-project";

#[derive(Debug, Clone)]
pub struct Identity {
    pub tenant: TenantId,
    pub project: Option<String>,
}

pub fn identify(headers: &HeaderMap) -> Identity {
    let tenant = headers
        .get(TENANT_HEADER)
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
        .map(TenantId::from)
        .unwrap_or_else(TenantId::default_tenant);
    let project = headers
        .get(PROJECT_HEADER)
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    Identity { tenant, project }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn defaults_when_missing() {
        let id = identify(&HeaderMap::new());
        assert_eq!(id.tenant, TenantId::default_tenant());
        assert!(id.project.is_none());
    }

    #[test]
    fn extracts_tenant_header() {
        let mut h = HeaderMap::new();
        h.insert(TENANT_HEADER, HeaderValue::from_static("alice"));
        h.insert(PROJECT_HEADER, HeaderValue::from_static("web-app"));
        let id = identify(&h);
        assert_eq!(id.tenant.as_str(), "alice");
        assert_eq!(id.project.as_deref(), Some("web-app"));
    }
}
