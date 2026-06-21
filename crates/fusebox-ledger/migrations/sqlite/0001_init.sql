-- Fusebox spend ledger — SQLite schema.
-- Idempotent: re-running this on each boot is safe.

CREATE TABLE IF NOT EXISTS spend_events (
    id                  TEXT PRIMARY KEY,
    ts                  TEXT NOT NULL,
    tenant_id           TEXT NOT NULL,
    provider            TEXT NOT NULL,
    model               TEXT NOT NULL,
    input_tokens        INTEGER NOT NULL DEFAULT 0,
    output_tokens       INTEGER NOT NULL DEFAULT 0,
    cache_read_tokens   INTEGER NOT NULL DEFAULT 0,
    cache_write_tokens  INTEGER NOT NULL DEFAULT 0,
    cost_usd            REAL NOT NULL DEFAULT 0.0,
    request_id          TEXT,
    status              TEXT NOT NULL DEFAULT 'completed',
    metadata            TEXT NOT NULL DEFAULT 'null'
);

-- Hot-path indexes: most reads are "spend in window for tenant".
CREATE INDEX IF NOT EXISTS idx_spend_tenant_ts
    ON spend_events (tenant_id, ts DESC);

CREATE INDEX IF NOT EXISTS idx_spend_ts
    ON spend_events (ts DESC);

CREATE TABLE IF NOT EXISTS breaker_events (
    id          TEXT PRIMARY KEY,
    ts          TEXT NOT NULL,
    tenant_id   TEXT NOT NULL,
    transition  TEXT NOT NULL,
    reason      TEXT
);

CREATE INDEX IF NOT EXISTS idx_breaker_tenant_ts
    ON breaker_events (tenant_id, ts DESC);

-- Budget increase request tracking table
CREATE TABLE IF NOT EXISTS budget_requests (
    id                  TEXT PRIMARY KEY,
    tenant_id           TEXT NOT NULL,
    window              TEXT NOT NULL,
    requested_limit_usd REAL NOT NULL,
    reason              TEXT,
    requested_at        TEXT NOT NULL,
    status              TEXT NOT NULL DEFAULT 'pending',
    decided_at          TEXT,
    decided_by          TEXT,
    decision_note       TEXT,
    ttl_seconds         INTEGER
);

CREATE INDEX IF NOT EXISTS idx_budget_req_tenant_status
    ON budget_requests (tenant_id, status);

CREATE INDEX IF NOT EXISTS idx_budget_req_requested_at
    ON budget_requests (requested_at DESC);
