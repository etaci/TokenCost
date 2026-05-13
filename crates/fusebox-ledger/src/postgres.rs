//! Postgres-backed ledger. Recommended once you outgrow a single box;
//! the bundled migration is idempotent and TimescaleDB-aware (the
//! hypertable promotion runs only when the extension is installed).

use crate::breaker_event::{BreakerEvent, BreakerTransitionKind};
use crate::event::{SpendEvent, SpendStatus};
use crate::store::{BreakerEventQuery, LedgerStore, SpendQuery, SpendTotals};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use fusebox_core::{CostUsd, FuseboxError, ModelId, Provider, Result, TenantId};
use sqlx::postgres::{PgPoolOptions, PgRow};
use sqlx::{PgPool, Row};
use std::time::Duration;
use uuid::Uuid;

const SCHEMA: &str = include_str!("../migrations/postgres/0001_init.sql");

#[derive(Debug, Clone)]
pub struct PgLedger {
    pool: PgPool,
}

impl PgLedger {
    /// Open a connection pool against `url` and apply migrations. The
    /// migration is idempotent so this is safe to call on every boot.
    pub async fn connect(url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(16)
            .min_connections(1)
            .acquire_timeout(Duration::from_secs(10))
            .idle_timeout(Some(Duration::from_secs(600)))
            .connect(url)
            .await
            .map_err(|e| FuseboxError::Storage(format!("postgres connect: {e}")))?;
        Self::from_pool(pool).await
    }

    /// Use an existing pool. Handy for tests that spin up `sqlx::PgPool`
    /// against `pg-embed` or testcontainers.
    pub async fn from_pool(pool: PgPool) -> Result<Self> {
        // Migrations are wrapped in a DO block (see postgres/0001_init.sql)
        // so we just execute the whole file.
        for stmt in split_sql(SCHEMA) {
            if stmt.trim().is_empty() {
                continue;
            }
            sqlx::query(&stmt)
                .execute(&pool)
                .await
                .map_err(|e| FuseboxError::Storage(format!("schema apply failed: {e}")))?;
        }
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

/// Split a multi-statement migration on bare semicolons that are not
/// inside a `$$ ... $$` block. Postgres rejects multi-statement strings
/// from the simple-query protocol when one of them is a DO block, so we
/// hand it the pieces one at a time.
fn split_sql(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_dollar = false;
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' && chars.peek() == Some(&'$') {
            chars.next();
            current.push_str("$$");
            in_dollar = !in_dollar;
            continue;
        }
        if c == ';' && !in_dollar {
            current.push(';');
            out.push(std::mem::take(&mut current));
            continue;
        }
        current.push(c);
    }
    if !current.trim().is_empty() {
        out.push(current);
    }
    out
}

#[async_trait]
impl LedgerStore for PgLedger {
    async fn record(&self, event: SpendEvent) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO spend_events (
                id, ts, tenant_id, provider, model,
                input_tokens, output_tokens, cache_read_tokens, cache_write_tokens,
                cost_usd, request_id, status, metadata
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
            "#,
        )
        .bind(event.id)
        .bind(event.ts)
        .bind(event.tenant_id.as_str())
        .bind(event.provider.as_str())
        .bind(event.model.as_str())
        .bind(event.input_tokens as i32)
        .bind(event.output_tokens as i32)
        .bind(event.cache_read_tokens as i32)
        .bind(event.cache_write_tokens as i32)
        .bind(event.cost_usd.0)
        .bind(event.request_id.as_deref())
        .bind(event.status.as_str())
        .bind(event.metadata.clone())
        .execute(&self.pool)
        .await
        .map_err(|e| FuseboxError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn totals(&self, query: &SpendQuery) -> Result<SpendTotals> {
        let until = query.until.unwrap_or_else(Utc::now);
        // We cast NUMERIC → DOUBLE PRECISION here so sqlx can bind into f64
        // without dragging in BigDecimal. Six decimals is plenty for USD.
        let row = match &query.tenant {
            Some(tenant) => sqlx::query(
                r#"
                SELECT
                  COALESCE(SUM(cost_usd), 0)::float8     AS cost,
                  COALESCE(SUM(input_tokens), 0)::bigint AS input_tokens,
                  COALESCE(SUM(output_tokens), 0)::bigint AS output_tokens,
                  COUNT(1)::bigint                       AS events
                FROM spend_events
                WHERE tenant_id = $1 AND ts >= $2 AND ts <= $3
                "#,
            )
            .bind(tenant.as_str())
            .bind(query.since)
            .bind(until)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| FuseboxError::Storage(e.to_string()))?,
            None => sqlx::query(
                r#"
                SELECT
                  COALESCE(SUM(cost_usd), 0)::float8     AS cost,
                  COALESCE(SUM(input_tokens), 0)::bigint AS input_tokens,
                  COALESCE(SUM(output_tokens), 0)::bigint AS output_tokens,
                  COUNT(1)::bigint                       AS events
                FROM spend_events
                WHERE ts >= $1 AND ts <= $2
                "#,
            )
            .bind(query.since)
            .bind(until)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| FuseboxError::Storage(e.to_string()))?,
        };
        Ok(SpendTotals {
            cost: CostUsd(row.try_get::<f64, _>("cost").unwrap_or(0.0)),
            input_tokens: row.try_get::<i64, _>("input_tokens").unwrap_or(0) as u64,
            output_tokens: row.try_get::<i64, _>("output_tokens").unwrap_or(0) as u64,
            events: row.try_get::<i64, _>("events").unwrap_or(0) as u64,
        })
    }

    async fn list(&self, query: &SpendQuery) -> Result<Vec<SpendEvent>> {
        let until = query.until.unwrap_or_else(Utc::now);
        let limit = query.limit.unwrap_or(100).min(1000) as i64;
        let rows = match &query.tenant {
            Some(tenant) => sqlx::query(
                r#"
                SELECT id, ts, tenant_id, provider, model,
                       input_tokens, output_tokens, cache_read_tokens, cache_write_tokens,
                       cost_usd::float8 AS cost_usd_f, request_id, status, metadata
                FROM spend_events
                WHERE tenant_id = $1 AND ts >= $2 AND ts <= $3
                ORDER BY ts DESC
                LIMIT $4
                "#,
            )
            .bind(tenant.as_str())
            .bind(query.since)
            .bind(until)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| FuseboxError::Storage(e.to_string()))?,
            None => sqlx::query(
                r#"
                SELECT id, ts, tenant_id, provider, model,
                       input_tokens, output_tokens, cache_read_tokens, cache_write_tokens,
                       cost_usd::float8 AS cost_usd_f, request_id, status, metadata
                FROM spend_events
                WHERE ts >= $1 AND ts <= $2
                ORDER BY ts DESC
                LIMIT $3
                "#,
            )
            .bind(query.since)
            .bind(until)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| FuseboxError::Storage(e.to_string()))?,
        };

        Ok(rows.into_iter().map(row_to_event).collect())
    }

    async fn ping(&self) -> Result<()> {
        sqlx::query("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| FuseboxError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn record_breaker(&self, event: BreakerEvent) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO breaker_events (id, ts, tenant_id, transition, reason)
            VALUES ($1,$2,$3,$4,$5)
            "#,
        )
        .bind(event.id)
        .bind(event.ts)
        .bind(event.tenant_id.as_str())
        .bind(event.transition.as_str())
        .bind(event.reason.as_deref())
        .execute(&self.pool)
        .await
        .map_err(|e| FuseboxError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn list_breaker_events(&self, query: &BreakerEventQuery) -> Result<Vec<BreakerEvent>> {
        let limit = query.limit.unwrap_or(200).min(1000) as i64;
        let rows = match &query.tenant {
            Some(tenant) => sqlx::query(
                r#"
                SELECT id, ts, tenant_id, transition, reason
                FROM breaker_events
                WHERE tenant_id = $1 AND ts >= $2
                ORDER BY ts DESC
                LIMIT $3
                "#,
            )
            .bind(tenant.as_str())
            .bind(query.since)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| FuseboxError::Storage(e.to_string()))?,
            None => sqlx::query(
                r#"
                SELECT id, ts, tenant_id, transition, reason
                FROM breaker_events
                WHERE ts >= $1
                ORDER BY ts DESC
                LIMIT $2
                "#,
            )
            .bind(query.since)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| FuseboxError::Storage(e.to_string()))?,
        };

        Ok(rows.into_iter().map(row_to_breaker_event).collect())
    }
}

fn row_to_event(row: PgRow) -> SpendEvent {
    let id: Uuid = row.try_get("id").unwrap_or_else(|_| Uuid::new_v4());
    let ts: DateTime<Utc> = row.try_get("ts").unwrap_or_else(|_| Utc::now());
    let tenant: String = row.try_get("tenant_id").unwrap_or_default();
    let provider: String = row.try_get("provider").unwrap_or_default();
    let model: String = row.try_get("model").unwrap_or_default();
    let input_tokens: i32 = row.try_get("input_tokens").unwrap_or(0);
    let output_tokens: i32 = row.try_get("output_tokens").unwrap_or(0);
    let cache_read: i32 = row.try_get("cache_read_tokens").unwrap_or(0);
    let cache_write: i32 = row.try_get("cache_write_tokens").unwrap_or(0);
    let cost_usd: f64 = row.try_get("cost_usd_f").unwrap_or(0.0);
    let request_id: Option<String> = row.try_get("request_id").ok();
    let status: String = row.try_get("status").unwrap_or_default();
    let metadata: serde_json::Value = row.try_get("metadata").unwrap_or(serde_json::Value::Null);

    SpendEvent {
        id,
        ts,
        tenant_id: TenantId::from(tenant),
        provider: Provider::from(provider.as_str()),
        model: ModelId::from(model),
        input_tokens: input_tokens.max(0) as u32,
        output_tokens: output_tokens.max(0) as u32,
        cache_read_tokens: cache_read.max(0) as u32,
        cache_write_tokens: cache_write.max(0) as u32,
        cost_usd: CostUsd(cost_usd),
        request_id,
        status: SpendStatus::from_str(&status),
        metadata,
    }
}

fn row_to_breaker_event(row: PgRow) -> BreakerEvent {
    let id: Uuid = row.try_get("id").unwrap_or_else(|_| Uuid::new_v4());
    let ts: DateTime<Utc> = row.try_get("ts").unwrap_or_else(|_| Utc::now());
    let tenant: String = row.try_get("tenant_id").unwrap_or_default();
    let transition: String = row.try_get("transition").unwrap_or_default();
    let reason: Option<String> = row.try_get("reason").ok();
    BreakerEvent {
        id,
        ts,
        tenant_id: TenantId::from(tenant),
        transition: BreakerTransitionKind::from_str(&transition),
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_sql_around_dollar_blocks() {
        let sql = "CREATE TABLE a (id INT); DO $$ BEGIN PERFORM 1; END $$; CREATE INDEX i ON a(id);";
        let stmts = split_sql(sql);
        assert_eq!(stmts.len(), 3);
        assert!(stmts[1].contains("DO $$"));
        assert!(stmts[1].contains("END $$"));
    }

    #[test]
    fn splits_sql_keeps_trailing_statement_without_semicolon() {
        let sql = "SELECT 1";
        assert_eq!(split_sql(sql), vec!["SELECT 1".to_string()]);
    }
}
