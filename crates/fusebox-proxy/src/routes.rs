//! HTTP routes.
//!
//! Layout:
//! - `GET  /health`                 — liveness for orchestrators
//! - `GET  /metrics`                — Prometheus exposition
//! - `GET  /v1/breaker/state`       — current breaker label for the caller's tenant
//! - `POST /v1/breaker/reset`       — operator override
//! - `GET  /v1/breakers`            — every tenant's breaker (control plane)
//! - `GET  /v1/spend`               — current-window spend + budget
//! - `GET  /v1/events`              — recent spend events (dashboard live tail)
//! - `GET  /v1/audit/breakers`      — breaker transition audit log
//! - `POST /v1/chat/completions`    — OpenAI-compatible passthrough
//! - `POST /v1/messages`            — Anthropic-compatible passthrough

use std::convert::Infallible;
use std::time::{Duration as StdDuration, Instant};

use axum::body::Body;
use axum::extract::{OriginalUri, Query, State};
use axum::http::{HeaderMap, Method, Response, StatusCode};
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use bytes::Bytes;
use chrono::{Duration as ChronoDuration, Utc};
use fusebox_core::{BudgetWindow, CostUsd, Decision, DenyReason, PricingTable, Provider, TenantId, TokenUsage};
use fusebox_ledger::event::SpendStatus;
use fusebox_ledger::{BreakerEventQuery, SpendEvent, SpendQuery};
use fusebox_policy::RequestEstimate;
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tracing::{debug, error, info, warn};

use crate::estimate::{parse_anthropic_messages, parse_openai_chat, ParsedRequest};
use crate::identity::{identify, Identity};
use crate::metrics::{names, render as render_metrics};
use crate::state::AppState;
use crate::stream_reconcile::reconcile;
use crate::upstream::{forward, parse_usage};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/metrics", get(prom_metrics))
        .route("/v1/breaker/state", get(breaker_state))
        .route("/v1/breaker/reset", post(breaker_reset))
        .route("/v1/breakers", get(breakers_list))
        .route("/v1/spend", get(spend_summary))
        .route("/v1/events", get(events_list))
        .route("/v1/events/stream", get(events_stream))
        .route("/v1/audit/breakers", get(audit_breakers))
        .route(
            "/v1/budget/requests",
            get(budget_requests_list).post(budget_requests_create),
        )
        .route("/v1/budget/requests/:id", get(budget_request_get))
        .route("/v1/budget/requests/:id/approve", post(budget_request_approve))
        .route("/v1/budget/requests/:id/reject", post(budget_request_reject))
        .route("/v1/mcp/budget", get(mcp_get_budget))
        .route("/v1/mcp/spend", get(mcp_get_spend))
        .route("/v1/mcp/breaker", get(mcp_get_breaker))
        .route("/v1/mcp/budget/request_increase", post(mcp_request_increase))
        .route("/v1/admin/reload", post(admin_reload))
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/completions", post(chat_completions))
        .route("/v1/messages", post(anthropic_messages))
        // OpenRouter is OpenAI-compatible — same body shape, different
        // upstream URL + pricing rows. Mount the same path family under
        // `/openrouter/` so clients can opt in by base URL change alone.
        .route("/openrouter/v1/chat/completions", post(openrouter_chat))
        // Google Gemini speaks `:generateContent` (and the streaming
        // `:streamGenerateContent`). The model lives in the URL path,
        // which we extract before parsing the body.
        .route("/v1beta/models/:model_action", post(google_generate))
        // Bedrock — passthrough only. Real SigV4 happens in a sidecar
        // (or the client). The route is here so operators can wire it
        // up to a signing proxy without forking Fusebox.
        .route("/bedrock/model/:model/invoke", post(bedrock_invoke))
        .route(
            "/bedrock/model/:model/invoke-with-response-stream",
            post(bedrock_invoke),
        )
        .with_state(state)
}

async fn health() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

async fn prom_metrics() -> impl IntoResponse {
    let body = render_metrics();
    (
        StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4")],
        body,
    )
}

async fn breaker_state(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let id = identify(&headers);
    let label = state.policy.breaker_label(&id.tenant);
    axum::Json(json!({ "tenant": id.tenant.as_str(), "state": label }))
}

#[derive(Debug, Default, Deserialize)]
struct BreakerResetRequest {
    /// Tenant whose breaker to reset. When omitted, derived from the
    /// `X-Fusebox-Tenant` header (or `default`).
    #[serde(default)]
    tenant: Option<String>,
}

async fn breaker_reset(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    // We parse the body manually so that callers can omit it entirely
    // (`curl -X POST /v1/breaker/reset` without a JSON document) and so
    // that we don't depend on `Option<Json<T>>` extractor behaviour that
    // shifted between axum 0.7 patch releases.
    let payload: BreakerResetRequest = if body.is_empty() {
        BreakerResetRequest::default()
    } else {
        serde_json::from_slice(&body).unwrap_or_default()
    };
    let body_tenant = payload.tenant.map(TenantId::from);
    let tenant = body_tenant.unwrap_or_else(|| identify(&headers).tenant);
    let trans = state.policy.manual_reset_audited(&tenant).await;
    info!(tenant = %tenant, "manual breaker reset via API");
    axum::Json(json!({
        "tenant": tenant.as_str(),
        "from": trans.from,
        "to": trans.to,
        "at": trans.at,
    }))
}

async fn breakers_list(State(state): State<AppState>) -> impl IntoResponse {
    let snap = state.policy.breaker_snapshot();
    let rows: Vec<_> = snap
        .into_iter()
        .map(|(t, s)| {
            json!({
                "tenant": t.as_str(),
                "state": s,
            })
        })
        .collect();
    axum::Json(json!({ "breakers": rows }))
}

#[derive(Debug, Deserialize)]
struct SpendQueryParams {
    /// Optional tenant filter; defaults to header / `default`.
    #[serde(default)]
    tenant: Option<String>,
    /// Window: `1m`, `1h`, `1d`, `1w`, `1mo`. Default `1d`.
    #[serde(default)]
    window: Option<String>,
}

async fn spend_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<SpendQueryParams>,
) -> Response<Body> {
    let tenant = q
        .tenant
        .map(TenantId::from)
        .unwrap_or_else(|| identify(&headers).tenant);
    let window = parse_window(q.window.as_deref()).unwrap_or(BudgetWindow::Day);

    let used = match state.policy.spend_for(&tenant, window).await {
        Ok(c) => c,
        Err(e) => return error_json(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };
    let budgets = state.policy.budgets_for_tenant(&tenant);
    let matching = budgets.iter().find(|b| b.window == window);
    let limit = matching.map(|b| b.limit_usd).unwrap_or(0.0);
    let fraction = if limit > 0.0 { used.dollars() / limit } else { 0.0 };

    let payload = json!({
        "tenant": tenant.as_str(),
        "window": window.as_label(),
        "used_usd": used.dollars(),
        "limit_usd": limit,
        "fraction": fraction,
        "budgets": budgets.iter().map(|b| json!({
            "limit_usd": b.limit_usd,
            "window": b.window.as_label(),
            "label": b.label,
        })).collect::<Vec<_>>(),
    });
    json_response(StatusCode::OK, &payload)
}

#[derive(Debug, Deserialize)]
struct EventsQueryParams {
    #[serde(default)]
    tenant: Option<String>,
    /// Lookback in seconds. Default 1 hour, max 7 days.
    #[serde(default)]
    seconds: Option<i64>,
    #[serde(default)]
    limit: Option<u32>,
}

async fn events_list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<EventsQueryParams>,
) -> Response<Body> {
    // Tenant filter: explicit param > header > all (no default header
    // shadowing here — events is an admin endpoint).
    let tenant = q.tenant.map(TenantId::from).or_else(|| {
        let id = identify(&headers);
        if id.tenant == TenantId::default_tenant() {
            None
        } else {
            Some(id.tenant)
        }
    });
    let seconds = q
        .seconds
        .unwrap_or(3600)
        .clamp(60, 7 * 24 * 3600);
    let limit = q.limit.unwrap_or(100).min(1000);

    let query = SpendQuery {
        tenant,
        since: Utc::now() - ChronoDuration::seconds(seconds),
        until: None,
        limit: Some(limit),
    };

    let events = match state.ledger.list(&query).await {
        Ok(e) => e,
        Err(e) => return error_json(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };
    let payload = json!({
        "events": events.iter().map(serialize_event).collect::<Vec<_>>(),
        "count": events.len(),
    });
    json_response(StatusCode::OK, &payload)
}

/// Live spend events streamed as Server-Sent Events. The dashboard and any
/// future MCP-aware agent can `EventSource('/v1/events/stream')` to get an
/// ongoing JSON event per recorded spend without polling.
///
/// Optional `?tenant=` filter. Slow consumers that fall behind by more than
/// the broadcast buffer get a single `lag` event and resume from current —
/// they can fill the gap by hitting `/v1/events` once.
async fn events_stream(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<EventsQueryParams>,
) -> Sse<impl Stream<Item = std::result::Result<SseEvent, Infallible>>> {
    let tenant_filter = q.tenant.map(TenantId::from).or_else(|| {
        let id = identify(&headers);
        if id.tenant == TenantId::default_tenant() {
            None
        } else {
            Some(id.tenant)
        }
    });

    let rx = state.events_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(move |item| match item {
        Ok(event) => {
            if let Some(t) = &tenant_filter {
                if &event.tenant_id != t {
                    return None;
                }
            }
            let payload = serialize_event(&event);
            Some(Ok(SseEvent::default()
                .event("spend")
                .id(event.id.to_string())
                .json_data(payload)
                .unwrap_or_else(|_| SseEvent::default().event("error").data("encode failed"))))
        }
        Err(BroadcastStreamRecvError::Lagged(missed)) => Some(Ok(SseEvent::default()
            .event("lag")
            .data(missed.to_string()))),
    });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(StdDuration::from_secs(15))
            .text("keep-alive"),
    )
}

async fn audit_breakers(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<EventsQueryParams>,
) -> Response<Body> {
    let tenant = q.tenant.map(TenantId::from).or_else(|| {
        let id = identify(&headers);
        if id.tenant == TenantId::default_tenant() {
            None
        } else {
            Some(id.tenant)
        }
    });
    let seconds = q.seconds.unwrap_or(24 * 3600).clamp(60, 30 * 24 * 3600);
    let limit = q.limit.unwrap_or(200).min(1000);

    let query = BreakerEventQuery {
        tenant,
        since: Utc::now() - ChronoDuration::seconds(seconds),
        limit: Some(limit),
    };
    let events = match state.ledger.list_breaker_events(&query).await {
        Ok(e) => e,
        Err(e) => return error_json(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };
    let payload = json!({
        "events": events.iter().map(|e| json!({
            "id": e.id,
            "ts": e.ts,
            "tenant": e.tenant_id.as_str(),
            "transition": e.transition.as_str(),
            "reason": e.reason,
        })).collect::<Vec<_>>(),
        "count": events.len(),
    });
    json_response(StatusCode::OK, &payload)
}

fn parse_window(s: Option<&str>) -> Option<BudgetWindow> {
    match s? {
        "1m" | "minute" | "min" => Some(BudgetWindow::Minute),
        "1h" | "hour" => Some(BudgetWindow::Hour),
        "1d" | "day" => Some(BudgetWindow::Day),
        "1w" | "week" => Some(BudgetWindow::Week),
        "1mo" | "month" => Some(BudgetWindow::Month),
        _ => None,
    }
}

#[derive(Serialize)]
struct EventDto<'a> {
    id: String,
    ts: chrono::DateTime<Utc>,
    tenant: &'a str,
    provider: &'a str,
    model: &'a str,
    input_tokens: u32,
    output_tokens: u32,
    cost_usd: f64,
    status: &'a str,
}

fn serialize_event(ev: &SpendEvent) -> serde_json::Value {
    let dto = EventDto {
        id: ev.id.to_string(),
        ts: ev.ts,
        tenant: ev.tenant_id.as_str(),
        provider: ev.provider.as_str(),
        model: ev.model.as_str(),
        input_tokens: ev.input_tokens,
        output_tokens: ev.output_tokens,
        cost_usd: ev.cost_usd.dollars(),
        status: ev.status.as_str(),
    };
    serde_json::to_value(dto).unwrap_or(serde_json::Value::Null)
}

// ---- Budget-increase request workflow ---------------------------------------

#[derive(Debug, Deserialize)]
struct BudgetRequestCreate {
    tenant: Option<String>,
    /// Window for the requested override: `1m|1h|1d|1w|1mo` or long form.
    window: Option<String>,
    /// Dollar amount, e.g. `100.0` for a $100 daily budget bump.
    requested_limit_usd: f64,
    #[serde(default)]
    reason: Option<String>,
    /// Optional self-expiry. Useful for "for the next 30 minutes" bumps.
    #[serde(default)]
    ttl_seconds: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
struct BudgetRequestDecision {
    #[serde(default)]
    actor: Option<String>,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BudgetRequestListParams {
    #[serde(default)]
    status: Option<String>,
}

async fn budget_requests_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<BudgetRequestCreate>,
) -> Response<Body> {
    let tenant = body
        .tenant
        .map(TenantId::from)
        .unwrap_or_else(|| identify(&headers).tenant);
    let window = parse_window(body.window.as_deref()).unwrap_or(BudgetWindow::Day);
    let req = crate::budget_requests::BudgetRequest::new(
        tenant,
        window,
        body.requested_limit_usd,
        body.reason,
        body.ttl_seconds,
    );
    match state.budget_requests.create(req) {
        Ok(saved) => json_response(StatusCode::CREATED, &serde_json::to_value(saved).unwrap()),
        Err(e) => error_json(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

async fn budget_requests_list(
    State(state): State<AppState>,
    Query(q): Query<BudgetRequestListParams>,
) -> Response<Body> {
    let status = q.status.as_deref().and_then(|s| match s {
        "pending" => Some(crate::budget_requests::RequestStatus::Pending),
        "approved" => Some(crate::budget_requests::RequestStatus::Approved),
        "rejected" => Some(crate::budget_requests::RequestStatus::Rejected),
        "expired" => Some(crate::budget_requests::RequestStatus::Expired),
        _ => None,
    });
    let items = state.budget_requests.list(status);
    let count = items.len();
    json_response(StatusCode::OK, &json!({ "requests": items, "count": count }))
}

async fn budget_request_get(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Response<Body> {
    let Ok(uuid) = uuid::Uuid::parse_str(&id) else {
        return error_json(StatusCode::BAD_REQUEST, "invalid id");
    };
    match state.budget_requests.get(uuid) {
        Some(r) => json_response(StatusCode::OK, &serde_json::to_value(r).unwrap()),
        None => error_json(StatusCode::NOT_FOUND, "not found"),
    }
}

async fn budget_request_approve(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    body: Bytes,
) -> Response<Body> {
    let Ok(uuid) = uuid::Uuid::parse_str(&id) else {
        return error_json(StatusCode::BAD_REQUEST, "invalid id");
    };
    let decision: BudgetRequestDecision = if body.is_empty() {
        BudgetRequestDecision::default()
    } else {
        serde_json::from_slice(&body).unwrap_or_default()
    };
    match state
        .budget_requests
        .approve(uuid, decision.actor, decision.note, &state.policy)
    {
        Ok(r) => {
            info!(tenant = %r.tenant, limit = r.requested_limit_usd, "budget request approved");
            json_response(StatusCode::OK, &serde_json::to_value(r).unwrap())
        }
        Err(crate::budget_requests::BudgetRequestError::NotFound(_)) => {
            error_json(StatusCode::NOT_FOUND, "not found")
        }
        Err(e) => error_json(StatusCode::CONFLICT, &e.to_string()),
    }
}

async fn budget_request_reject(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    body: Bytes,
) -> Response<Body> {
    let Ok(uuid) = uuid::Uuid::parse_str(&id) else {
        return error_json(StatusCode::BAD_REQUEST, "invalid id");
    };
    let decision: BudgetRequestDecision = if body.is_empty() {
        BudgetRequestDecision::default()
    } else {
        serde_json::from_slice(&body).unwrap_or_default()
    };
    match state
        .budget_requests
        .reject(uuid, decision.actor, decision.note)
    {
        Ok(r) => json_response(StatusCode::OK, &serde_json::to_value(r).unwrap()),
        Err(crate::budget_requests::BudgetRequestError::NotFound(_)) => {
            error_json(StatusCode::NOT_FOUND, "not found")
        }
        Err(e) => error_json(StatusCode::CONFLICT, &e.to_string()),
    }
}

// ---- MCP tool surface (HTTP wrappers the @fusebox/mcp server will call) ----
//
// The agent-side MCP server (TypeScript, lives in `packages/mcp-server/`)
// translates each of these into a tool the LLM can call. Keeping the
// translation layer over HTTP means the MCP server can run in its own
// process — perfect for the "agent runs locally, Fusebox runs on a server"
// deployment story in `架构.md`.

#[derive(Debug, Deserialize)]
struct McpTenantQuery {
    #[serde(default)]
    tenant: Option<String>,
    #[serde(default)]
    window: Option<String>,
}

async fn mcp_get_budget(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<McpTenantQuery>,
) -> Response<Body> {
    let tenant = q
        .tenant
        .map(TenantId::from)
        .unwrap_or_else(|| identify(&headers).tenant);
    let window = parse_window(q.window.as_deref()).unwrap_or(BudgetWindow::Day);
    let used = match state.policy.spend_for(&tenant, window).await {
        Ok(c) => c,
        Err(e) => return error_json(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };
    let budgets = state.policy.budgets_for_tenant(&tenant);
    let matching = budgets.iter().find(|b| b.window == window);
    let limit = matching.map(|b| b.limit_usd).unwrap_or(0.0);
    let remaining = (limit - used.dollars()).max(0.0);
    json_response(
        StatusCode::OK,
        &json!({
            "tenant": tenant.as_str(),
            "window": window.as_label(),
            "limit_usd": limit,
            "used_usd": used.dollars(),
            "remaining_usd": remaining,
            "fraction_used": if limit > 0.0 { used.dollars() / limit } else { 0.0 },
        }),
    )
}

async fn mcp_get_spend(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<McpTenantQuery>,
) -> Response<Body> {
    let tenant = q
        .tenant
        .map(TenantId::from)
        .unwrap_or_else(|| identify(&headers).tenant);
    let window = parse_window(q.window.as_deref()).unwrap_or(BudgetWindow::Day);
    let used = match state.policy.spend_for(&tenant, window).await {
        Ok(c) => c,
        Err(e) => return error_json(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };
    json_response(
        StatusCode::OK,
        &json!({
            "tenant": tenant.as_str(),
            "window": window.as_label(),
            "used_usd": used.dollars(),
        }),
    )
}

async fn mcp_get_breaker(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<McpTenantQuery>,
) -> Response<Body> {
    let tenant = q
        .tenant
        .map(TenantId::from)
        .unwrap_or_else(|| identify(&headers).tenant);
    let breaker_state = state.policy.breaker_state(&tenant);
    let label = breaker_state.label();
    let cooldown_remaining_secs = match &breaker_state {
        fusebox_policy::BreakerState::Open { cooldown_until, .. } => {
            let now = Utc::now();
            if *cooldown_until > now {
                (*cooldown_until - now).num_seconds()
            } else {
                0
            }
        }
        _ => 0,
    };
    json_response(
        StatusCode::OK,
        &json!({
            "tenant": tenant.as_str(),
            "state": label,
            "cooldown_remaining_secs": cooldown_remaining_secs,
            "details": breaker_state,
        }),
    )
}

#[derive(Debug, Deserialize)]
struct McpRequestIncrease {
    #[serde(default)]
    tenant: Option<String>,
    #[serde(default)]
    window: Option<String>,
    requested_limit_usd: f64,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    ttl_seconds: Option<u64>,
}

async fn mcp_request_increase(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<McpRequestIncrease>,
) -> Response<Body> {
    let tenant = body
        .tenant
        .map(TenantId::from)
        .unwrap_or_else(|| identify(&headers).tenant);
    let window = parse_window(body.window.as_deref()).unwrap_or(BudgetWindow::Day);
    let req = crate::budget_requests::BudgetRequest::new(
        tenant,
        window,
        body.requested_limit_usd,
        body.reason,
        body.ttl_seconds,
    );
    match state.budget_requests.create(req) {
        Ok(saved) => json_response(
            StatusCode::ACCEPTED,
            &json!({
                "id": saved.id,
                "status": saved.status.as_str(),
                "tenant": saved.tenant.as_str(),
                "window": saved.window.as_label(),
                "requested_limit_usd": saved.requested_limit_usd,
                "message": "request filed — awaiting operator approval",
            }),
        ),
        Err(e) => error_json(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

async fn chat_completions(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> Response<Body> {
    proxy_request(state, uri.to_string(), headers, body, ProviderKind::OpenAI).await
}

/// Trigger an on-disk config + pricing reload. Equivalent to sending
/// SIGHUP on Unix; the only mutation-capable admin endpoint. Returns the
/// resulting model count + provider count so operators can sanity-check
/// the reload from the response alone.
async fn admin_reload(State(state): State<AppState>) -> Response<Body> {
    match crate::admin::reload(&state).await {
        Ok(summary) => json_response(
            StatusCode::OK,
            &serde_json::to_value(summary).unwrap_or(serde_json::Value::Null),
        ),
        Err(e) => error_json(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn anthropic_messages(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> Response<Body> {
    proxy_request(state, uri.to_string(), headers, body, ProviderKind::Anthropic).await
}

async fn openrouter_chat(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> Response<Body> {
    // Strip the `/openrouter` prefix before forwarding so the upstream
    // path matches OpenRouter's API ("/api/v1/chat/completions").
    let path = uri
        .to_string()
        .strip_prefix("/openrouter")
        .map(|s| s.to_string())
        .unwrap_or_else(|| uri.to_string());
    proxy_request(state, path, headers, body, ProviderKind::OpenRouter).await
}

async fn google_generate(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    axum::extract::Path(model_action): axum::extract::Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response<Body> {
    // model_action looks like "gemini-1.5-pro:generateContent" or
    // "gemini-1.5-pro:streamGenerateContent". Split into (model, action).
    let (model, action) = match model_action.rsplit_once(':') {
        Some(parts) => parts,
        None => return error_json(StatusCode::BAD_REQUEST, "expected `<model>:<action>` path"),
    };
    let streaming = action == "streamGenerateContent";
    // Inject the model + stream flag into the body so the shared estimator
    // can do its job. We re-serialise — small payloads, negligible cost.
    let mut json: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => serde_json::Value::Object(Default::default()),
    };
    if let Some(map) = json.as_object_mut() {
        map.insert("model".into(), serde_json::Value::String(model.to_string()));
    }
    let synthetic = match serde_json::to_vec(&json) {
        Ok(b) => Bytes::from(b),
        Err(e) => return error_json(StatusCode::BAD_REQUEST, &e.to_string()),
    };
    // After the parser inspects the synthetic body, we still forward the
    // *original* request bytes upstream — Google rejects the synthetic
    // `model` field. The proxy_request helper buffers the body once; we
    // refactor minimally by parsing with the synthetic copy then handing
    // the originals back.
    let parsed_route = uri.to_string();
    proxy_request_with_alt_parse_body(
        state,
        parsed_route,
        headers,
        body,
        synthetic,
        ProviderKind::Google,
        streaming,
    )
    .await
}

async fn bedrock_invoke(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    axum::extract::Path(model): axum::extract::Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response<Body> {
    // Bedrock's `model/{id}/invoke` body shape varies by model family.
    // We assume Anthropic-shaped JSON (the most common Bedrock workload)
    // and let the estimator try its best; for non-Anthropic families the
    // parser returns an empty estimate and the policy still works
    // (budget gating still applies, just with $0 pre-flight).
    let mut json: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => serde_json::Value::Object(Default::default()),
    };
    if let Some(map) = json.as_object_mut() {
        map.entry("model")
            .or_insert(serde_json::Value::String(model.clone()));
    }
    let synthetic = match serde_json::to_vec(&json) {
        Ok(b) => Bytes::from(b),
        Err(e) => return error_json(StatusCode::BAD_REQUEST, &e.to_string()),
    };
    proxy_request_with_alt_parse_body(
        state,
        uri.to_string(),
        headers,
        body,
        synthetic,
        ProviderKind::Bedrock,
        false,
    )
    .await
}

#[derive(Debug, Clone, Copy)]
enum ProviderKind {
    OpenAI,
    Anthropic,
    /// OpenAI-compatible. Pricing keys come from `pricing/openrouter.yaml`
    /// but the *request* shape is identical to OpenAI, so we reuse the
    /// estimator and only swap the upstream base URL.
    OpenRouter,
    /// Google Gemini. The `:generateContent` shape is different enough
    /// that we currently skip token estimation (charge $0 pre-flight) and
    /// reconcile from the upstream `usageMetadata` block post-flight.
    Google,
    /// Bedrock — included so the config-discovery story is complete. The
    /// route forwards bytes unchanged; AWS SigV4 must be applied upstream
    /// (e.g. via a sidecar). Cost is read from the upstream response when
    /// the model speaks Anthropic-flavoured JSON; otherwise $0 until a
    /// per-model adapter ships.
    Bedrock,
}

impl ProviderKind {
    fn provider(self) -> Provider {
        match self {
            ProviderKind::OpenAI => Provider::OpenAI,
            ProviderKind::Anthropic => Provider::Anthropic,
            ProviderKind::OpenRouter => Provider::OpenRouter,
            ProviderKind::Google => Provider::Google,
            ProviderKind::Bedrock => Provider::Bedrock,
        }
    }

    fn config_key(self) -> &'static str {
        match self {
            ProviderKind::OpenAI => "openai",
            ProviderKind::Anthropic => "anthropic",
            ProviderKind::OpenRouter => "openrouter",
            ProviderKind::Google => "google",
            ProviderKind::Bedrock => "bedrock",
        }
    }

    fn parse(self, body: &[u8], pricing: &PricingTable) -> Result<ParsedRequest, String> {
        match self {
            ProviderKind::OpenAI | ProviderKind::OpenRouter => parse_openai_chat(body, pricing),
            ProviderKind::Anthropic | ProviderKind::Bedrock => {
                parse_anthropic_messages(body, pricing)
            }
            ProviderKind::Google => crate::estimate::parse_google_generate_content(body, pricing),
        }
    }

    /// Stream flavor for SSE reconciliation. OpenAI / OpenRouter share
    /// the OpenAI flavor; Anthropic / Bedrock share the Anthropic flavor.
    /// Google falls back to OpenAI flavor — it's not strictly accurate
    /// but is harmless when no `usage` block matches.
    fn stream_flavor(self) -> crate::stream_reconcile::StreamFlavor {
        match self {
            ProviderKind::Anthropic | ProviderKind::Bedrock => {
                crate::stream_reconcile::StreamFlavor::Anthropic
            }
            _ => crate::stream_reconcile::StreamFlavor::OpenAi,
        }
    }
}

async fn proxy_request(
    state: AppState,
    path_and_query: String,
    headers: HeaderMap,
    body: Bytes,
    kind: ProviderKind,
) -> Response<Body> {
    let id = identify(&headers);
    metrics::counter!(names::REQUESTS_TOTAL, "tenant" => id.tenant.to_string()).increment(1);

    // 1. Parse + estimate.
    let pricing = state.pricing.load();
    let parsed = match kind.parse(&body, &pricing) {
        Ok(p) => p,
        Err(msg) => {
            warn!(tenant = %id.tenant, "rejected: {msg}");
            return error_json(StatusCode::BAD_REQUEST, &msg);
        }
    };
    proxy_request_inner(state, path_and_query, headers, body, parsed, id, kind).await
}

/// Alternate entry point used when the body the client sent isn't the body
/// the estimator can parse — e.g. Google Gemini, where the model id lives
/// in the URL path. The caller hands us:
///   - `body`       : the bytes we forward to the upstream verbatim
///   - `parse_body` : a synthetic copy with extra fields (like `model`)
///                    injected so the shared estimator works
async fn proxy_request_with_alt_parse_body(
    state: AppState,
    path_and_query: String,
    headers: HeaderMap,
    body: Bytes,
    parse_body: Bytes,
    kind: ProviderKind,
    streaming_hint: bool,
) -> Response<Body> {
    let id = identify(&headers);
    metrics::counter!(names::REQUESTS_TOTAL, "tenant" => id.tenant.to_string()).increment(1);

    let pricing = state.pricing.load();
    let mut parsed = match kind.parse(&parse_body, &pricing) {
        Ok(p) => p,
        Err(msg) => {
            warn!(tenant = %id.tenant, "rejected: {msg}");
            return error_json(StatusCode::BAD_REQUEST, &msg);
        }
    };
    // Routes that streamGenerateContent or invoke-with-response-stream
    // can't carry `"stream": true` in the body. Honor an explicit hint
    // so the response tap kicks in.
    if streaming_hint {
        parsed.is_streaming = true;
    }
    proxy_request_inner(state, path_and_query, headers, body, parsed, id, kind).await
}

async fn proxy_request_inner(
    state: AppState,
    path_and_query: String,
    headers: HeaderMap,
    body: Bytes,
    parsed: ParsedRequest,
    id: Identity,
    kind: ProviderKind,
) -> Response<Body> {
    let started = Instant::now();

    debug!(
        tenant = %id.tenant,
        model = %parsed.model,
        est_input = parsed.estimated_usage.input_tokens,
        est_output = parsed.estimated_usage.output_tokens,
        est_cost = parsed.estimated_cost.dollars(),
        streaming = parsed.is_streaming,
        "pre-flight"
    );

    let estimate = RequestEstimate::new(
        kind.provider(),
        parsed.model.clone(),
        parsed.estimated_usage,
        parsed.estimated_cost,
    )
    .streaming(parsed.is_streaming);

    // 2. Policy pre-flight.
    let decision = match state.policy.evaluate(&id.tenant, &estimate).await {
        Ok(d) => d,
        Err(e) => {
            error!(tenant = %id.tenant, "policy error: {e}");
            return error_json(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string());
        }
    };

    if let Decision::Deny(reason) = &decision {
        record_blocked(&state, &id, &parsed, kind.provider(), reason).await;
        metrics::counter!(names::DENIED_TOTAL, "tenant" => id.tenant.to_string()).increment(1);
        return deny_response(reason);
    }

    // 3. Forward to upstream.
    let base_url = match state.config.load().providers.get(kind.config_key()) {
        Some(p) => p.base_url.clone(),
        None => {
            error!("no provider config for {}", kind.config_key());
            return error_json(StatusCode::INTERNAL_SERVER_ERROR, "provider not configured");
        }
    };

    let upstream = forward(
        &state.http,
        &base_url,
        &path_and_query,
        Method::POST,
        &headers,
        body,
    )
    .await;

    let upstream_resp = match upstream {
        Ok(r) => r,
        Err(msg) => {
            metrics::counter!(names::UPSTREAM_FAILED_TOTAL).increment(1);
            warn!(tenant = %id.tenant, "upstream call failed: {msg}");
            // Tell the policy engine the call failed so a half-open breaker
            // re-trips instead of optimistically closing.
            let _ = state
                .policy
                .record_outcome(&id.tenant, CostUsd::zero(), false)
                .await;
            return error_json(StatusCode::BAD_GATEWAY, &msg);
        }
    };

    let status = upstream_resp.status();
    let resp_headers = upstream_resp.headers().clone();

    // Streaming pass-through: tee bytes to both the client and a server-
    // side SSE parser so the ledger can charge actual usage instead of the
    // pre-flight estimate. The parser falls back to the estimate when the
    // upstream omits a `usage` block (older OpenAI clients without
    // `stream_options.include_usage`, exotic providers, …).
    if parsed.is_streaming {
        let bytes_stream = upstream_resp.bytes_stream();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Bytes>();

        let flavor = kind.stream_flavor();

        // Background reconciler. Owns the receiver; ends when `tx` drops,
        // which happens once the tapped stream is fully consumed *or* the
        // client disconnects (whichever comes first).
        let st_recon = state.clone();
        let parsed_recon = parsed.clone();
        let id_recon = id.clone();
        let provider_kind = kind.provider();
        tokio::spawn(async move {
            let mut usage = reconcile(rx, flavor).await;
            // Fall back to the pre-flight estimate when the upstream
            // didn't surface a `usage` block at all.
            if usage.total() == 0 {
                usage = parsed_recon.estimated_usage;
            }
            let cost = st_recon
                .pricing
                .load()
                .cost_for(&parsed_recon.model, &usage)
                .unwrap_or(parsed_recon.estimated_cost);
            record_completed(
                &st_recon,
                &id_recon,
                &parsed_recon,
                provider_kind,
                usage,
                cost,
            )
            .await;
            let _ = st_recon
                .policy
                .record_outcome(&id_recon.tenant, cost, true)
                .await;
        });

        // Tap: forward each chunk to the client *and* hand a copy to the
        // reconciler. `send` is a no-op when the receiver was already
        // dropped (e.g. parser exited early), so this can't fail the
        // request.
        let tapped = bytes_stream.map(move |chunk| {
            if let Ok(b) = &chunk {
                let _ = tx.send(b.clone());
            }
            chunk
        });

        let body_out = Body::from_stream(tapped);
        return build_response(map_status(status), &resp_headers, body_out);
    }

    // Non-streaming: buffer the body, parse usage, charge real cost.
    let body_bytes = match upstream_resp.bytes().await {
        Ok(b) => b,
        Err(e) => {
            metrics::counter!(names::UPSTREAM_FAILED_TOTAL).increment(1);
            warn!("upstream body read failed: {e}");
            let _ = state
                .policy
                .record_outcome(&id.tenant, CostUsd::zero(), false)
                .await;
            return error_json(StatusCode::BAD_GATEWAY, &e.to_string());
        }
    };

    let succeeded = status.is_success();
    let real_usage = if succeeded {
        let u = parse_usage(&body_bytes, kind.provider());
        if u.total() > 0 {
            u
        } else {
            parsed.estimated_usage
        }
    } else {
        TokenUsage::default()
    };

    let real_cost = if succeeded {
        state
            .pricing
            .load()
            .cost_for(&parsed.model, &real_usage)
            .unwrap_or(parsed.estimated_cost)
    } else {
        CostUsd::zero()
    };

    record_completed(
        &state,
        &id,
        &parsed,
        kind.provider(),
        real_usage,
        real_cost,
    )
    .await;
    let _ = state
        .policy
        .record_outcome(&id.tenant, real_cost, succeeded)
        .await;

    info!(
        tenant = %id.tenant,
        model = %parsed.model,
        elapsed_ms = started.elapsed().as_millis() as u64,
        cost = real_cost.dollars(),
        in_tokens = real_usage.input_tokens,
        out_tokens = real_usage.output_tokens,
        "completed"
    );

    build_response(map_status(status), &resp_headers, Body::from(body_bytes))
}

fn build_response(status: StatusCode, src_headers: &reqwest::header::HeaderMap, body: Body) -> Response<Body> {
    let mut builder = Response::builder().status(status);
    if let Some(h) = builder.headers_mut() {
        for (k, v) in src_headers.iter() {
            if is_passthrough(k.as_str()) {
                if let Ok(name) = axum::http::HeaderName::from_bytes(k.as_str().as_bytes()) {
                    if let Ok(val) = axum::http::HeaderValue::from_bytes(v.as_bytes()) {
                        h.insert(name, val);
                    }
                }
            }
        }
    }
    builder.body(body).unwrap_or_else(|_| empty_response())
}

fn is_passthrough(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "content-type" | "cache-control" | "x-request-id" | "openai-version" | "anthropic-version"
    )
}

fn map_status(s: reqwest::StatusCode) -> StatusCode {
    StatusCode::from_u16(s.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY)
}

fn deny_response(reason: &DenyReason) -> Response<Body> {
    let body = json!({
        "error": {
            "type": "fusebox_denied",
            "code": reason_code(reason),
            "message": reason.as_label(),
        }
    });
    let bytes = serde_json::to_vec(&body).unwrap_or_default();
    Response::builder()
        .status(StatusCode::TOO_MANY_REQUESTS)
        .header("content-type", "application/json")
        .body(Body::from(bytes))
        .unwrap_or_else(|_| empty_response())
}

fn reason_code(reason: &DenyReason) -> &'static str {
    match reason {
        DenyReason::BudgetExceeded { .. } => "budget_exceeded",
        DenyReason::BreakerOpen => "breaker_open",
        DenyReason::AnomalyDetected => "anomaly_detected",
        DenyReason::RateLimit => "rate_limit",
        DenyReason::PerRequestCostTooHigh => "per_request_cost_too_high",
        DenyReason::Manual => "manual",
    }
}

fn error_json(status: StatusCode, msg: &str) -> Response<Body> {
    let body = json!({ "error": { "message": msg } });
    json_response(status, &body)
}

fn json_response(status: StatusCode, body: &serde_json::Value) -> Response<Body> {
    let bytes = serde_json::to_vec(body).unwrap_or_default();
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Body::from(bytes))
        .unwrap_or_else(|_| empty_response())
}

fn empty_response() -> Response<Body> {
    Response::new(Body::empty())
}

async fn record_completed(
    state: &AppState,
    id: &Identity,
    parsed: &ParsedRequest,
    provider: Provider,
    usage: TokenUsage,
    cost: CostUsd,
) {
    let mut event = SpendEvent::now(
        id.tenant.clone(),
        provider,
        parsed.model.clone(),
        usage,
        cost,
        SpendStatus::Completed,
    );
    if let Some(p) = &id.project {
        event = event.with_metadata(json!({ "project": p }));
    }
    if let Err(e) = state.ledger.record(event.clone()).await {
        error!("ledger record failed: {e}");
    }
    state.publish_event(event);
    // metrics::counter expects u64; multiply by 1M to keep 6-decimal precision.
    metrics::counter!(names::COST_USD, "tenant" => id.tenant.to_string())
        .increment((cost.dollars() * 1_000_000.0) as u64);
    metrics::counter!(names::TOKENS_TOTAL, "tenant" => id.tenant.to_string(), "kind" => "input")
        .increment(usage.input_tokens as u64);
    metrics::counter!(names::TOKENS_TOTAL, "tenant" => id.tenant.to_string(), "kind" => "output")
        .increment(usage.output_tokens as u64);
}

async fn record_blocked(
    state: &AppState,
    id: &Identity,
    parsed: &ParsedRequest,
    provider: Provider,
    reason: &DenyReason,
) {
    let event = SpendEvent::now(
        id.tenant.clone(),
        provider,
        parsed.model.clone(),
        TokenUsage::default(),
        CostUsd::zero(),
        SpendStatus::Blocked,
    )
    .with_metadata(json!({
        "reason": reason_code(reason),
        "label": reason.as_label(),
        "project": id.project,
    }));
    if let Err(e) = state.ledger.record(event.clone()).await {
        error!("ledger record (blocked) failed: {e}");
    }
    state.publish_event(event);
}
