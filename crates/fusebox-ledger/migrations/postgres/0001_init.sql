-- Fusebox spend ledger — Postgres schema.
--
-- TimescaleDB is optional. If the extension is present we promote the
-- spend_events table into a hypertable; if not, we fall back to a normal
-- Postgres table with the same columns.

CREATE TABLE IF NOT EXISTS spend_events (
    id                  UUID PRIMARY KEY,
    ts                  TIMESTAMPTZ NOT NULL,
    tenant_id           TEXT NOT NULL,
    provider            TEXT NOT NULL,
    model               TEXT NOT NULL,
    input_tokens        INTEGER NOT NULL DEFAULT 0,
    output_tokens       INTEGER NOT NULL DEFAULT 0,
    cache_read_tokens   INTEGER NOT NULL DEFAULT 0,
    cache_write_tokens  INTEGER NOT NULL DEFAULT 0,
    cost_usd            NUMERIC(14, 6) NOT NULL DEFAULT 0,
    request_id          TEXT,
    status              TEXT NOT NULL DEFAULT 'completed',
    metadata            JSONB NOT NULL DEFAULT 'null'::jsonb
);

CREATE INDEX IF NOT EXISTS idx_spend_tenant_ts
    ON spend_events (tenant_id, ts DESC);

CREATE INDEX IF NOT EXISTS idx_spend_ts
    ON spend_events (ts DESC);

CREATE TABLE IF NOT EXISTS breaker_events (
    id          UUID PRIMARY KEY,
    ts          TIMESTAMPTZ NOT NULL,
    tenant_id   TEXT NOT NULL,
    transition  TEXT NOT NULL,
    reason      TEXT
);

CREATE INDEX IF NOT EXISTS idx_breaker_tenant_ts
    ON breaker_events (tenant_id, ts DESC);

-- Optional TimescaleDB hypertable promotion. Wrapped in DO block so the
-- migration is a no-op when the extension is missing.
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'timescaledb') THEN
        PERFORM create_hypertable('spend_events', 'ts',
            if_not_exists => TRUE,
            migrate_data  => TRUE);
    END IF;
END $$;
