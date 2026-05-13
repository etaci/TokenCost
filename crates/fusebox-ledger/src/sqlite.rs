//! SQLite-backed ledger. Default for `fusebox start` so users never have to
//! stand up Postgres just to try the product.

use crate::breaker_event::{BreakerEvent, BreakerTransitionKind};
use crate::event::{SpendEvent, SpendStatus};
use crate::store::{BreakerEventQuery, LedgerStore, SpendQuery, SpendTotals};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use fusebox_core::{CostUsd, FuseboxError, ModelId, Provider, Result, TenantId};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Row, SqlitePool};
use std::path::Path;
use std::str::FromStr;
use uuid::Uuid;

const SCHEMA: &str = include_str!("../migrations/sqlite/0001_init.sql");

#[derive(Debug, Clone)]
pub struct SqliteLedger {
    pool: SqlitePool,
}

impl SqliteLedger {
    /// Open (or create) a SQLite ledger at `path`. Parent directories are
    /// created if missing — matches the indie "just works" promise.
    pub async fn connect(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let url = format!("sqlite://{}?mode=rwc", path.display());
        let opts = SqliteConnectOptions::from_str(&url)
            .map_err(|e| FuseboxError::Storage(e.to_string()))?
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(std::time::Duration::from_secs(5));

        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .connect_with(opts)
            .await
            .map_err(|e| FuseboxError::Storage(e.to_string()))?;

        // Hand-rolled migration: SCHEMA is idempotent (CREATE TABLE IF NOT
        // EXISTS), so we just run it on every boot.
        sqlx::query(SCHEMA)
            .execute(&pool)
            .await
            .map_err(|e| FuseboxError::Storage(format!("schema apply failed: {e}")))?;

        Ok(Self { pool })
    }

    /// In-memory SQLite for tests.
    pub async fn in_memory() -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .map_err(|e| FuseboxError::Storage(e.to_string()))?;
        sqlx::query(SCHEMA)
            .execute(&pool)
            .await
            .map_err(|e| FuseboxError::Storage(e.to_string()))?;
        Ok(Self { pool })
    }
}

#[async_trait]
impl LedgerStore for SqliteLedger {
    async fn record(&self, event: SpendEvent) -> Result<()> {
        let metadata = serde_json::to_string(&event.metadata).unwrap_or_else(|_| "null".into());
        sqlx::query(
            r#"
            INSERT INTO spend_events (
                id, ts, tenant_id, provider, model,
                input_tokens, output_tokens, cache_read_tokens, cache_write_tokens,
                cost_usd, request_id, status, metadata
            ) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?)
            "#,
        )
        .bind(event.id.to_string())
        .bind(event.ts.to_rfc3339())
        .bind(event.tenant_id.as_str())
        .bind(event.provider.as_str())
        .bind(event.model.as_str())
        .bind(event.input_tokens as i64)
        .bind(event.output_tokens as i64)
        .bind(event.cache_read_tokens as i64)
        .bind(event.cache_write_tokens as i64)
        .bind(event.cost_usd.0)
        .bind(event.request_id.as_deref())
        .bind(event.status.as_str())
        .bind(metadata)
        .execute(&self.pool)
        .await
        .map_err(|e| FuseboxError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn totals(&self, query: &SpendQuery) -> Result<SpendTotals> {
        let until = query.until.unwrap_or_else(Utc::now);
        let row = match &query.tenant {
            Some(tenant) => sqlx::query(
                r#"
                SELECT
                  COALESCE(SUM(cost_usd), 0)         AS cost,
                  COALESCE(SUM(input_tokens), 0)     AS input_tokens,
                  COALESCE(SUM(output_tokens), 0)    AS output_tokens,
                  COUNT(1)                           AS events
                FROM spend_events
                WHERE tenant_id = ? AND ts >= ? AND ts <= ?
                "#,
            )
            .bind(tenant.as_str())
            .bind(query.since.to_rfc3339())
            .bind(until.to_rfc3339())
            .fetch_one(&self.pool)
            .await
            .map_err(|e| FuseboxError::Storage(e.to_string()))?,
            None => sqlx::query(
                r#"
                SELECT
                  COALESCE(SUM(cost_usd), 0)         AS cost,
                  COALESCE(SUM(input_tokens), 0)     AS input_tokens,
                  COALESCE(SUM(output_tokens), 0)    AS output_tokens,
                  COUNT(1)                           AS events
                FROM spend_events
                WHERE ts >= ? AND ts <= ?
                "#,
            )
            .bind(query.since.to_rfc3339())
            .bind(until.to_rfc3339())
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
                       cost_usd, request_id, status, metadata
                FROM spend_events
                WHERE tenant_id = ? AND ts >= ? AND ts <= ?
                ORDER BY ts DESC
                LIMIT ?
                "#,
            )
            .bind(tenant.as_str())
            .bind(query.since.to_rfc3339())
            .bind(until.to_rfc3339())
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| FuseboxError::Storage(e.to_string()))?,
            None => sqlx::query(
                r#"
                SELECT id, ts, tenant_id, provider, model,
                       input_tokens, output_tokens, cache_read_tokens, cache_write_tokens,
                       cost_usd, request_id, status, metadata
                FROM spend_events
                WHERE ts >= ? AND ts <= ?
                ORDER BY ts DESC
                LIMIT ?
                "#,
            )
            .bind(query.since.to_rfc3339())
            .bind(until.to_rfc3339())
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| FuseboxError::Storage(e.to_string()))?,
        };

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let id_str: String = row
                .try_get("id")
                .map_err(|e| FuseboxError::Storage(e.to_string()))?;
            let ts_str: String = row
                .try_get("ts")
                .map_err(|e| FuseboxError::Storage(e.to_string()))?;
            let metadata_str: String = row.try_get("metadata").unwrap_or_else(|_| "null".into());
            let provider_str: String = row.try_get("provider").unwrap_or_default();
            let status_str: String = row.try_get("status").unwrap_or_default();

            let event = SpendEvent {
                id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
                ts: DateTime::parse_from_rfc3339(&ts_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                tenant_id: TenantId::from(row.try_get::<String, _>("tenant_id").unwrap_or_default()),
                provider: Provider::from(provider_str.as_str()),
                model: ModelId::from(row.try_get::<String, _>("model").unwrap_or_default()),
                input_tokens: row.try_get::<i64, _>("input_tokens").unwrap_or(0) as u32,
                output_tokens: row.try_get::<i64, _>("output_tokens").unwrap_or(0) as u32,
                cache_read_tokens: row.try_get::<i64, _>("cache_read_tokens").unwrap_or(0) as u32,
                cache_write_tokens: row.try_get::<i64, _>("cache_write_tokens").unwrap_or(0) as u32,
                cost_usd: CostUsd(row.try_get::<f64, _>("cost_usd").unwrap_or(0.0)),
                request_id: row.try_get("request_id").ok(),
                status: SpendStatus::from_str(&status_str),
                metadata: serde_json::from_str(&metadata_str).unwrap_or(serde_json::Value::Null),
            };
            out.push(event);
        }
        Ok(out)
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
            VALUES (?,?,?,?,?)
            "#,
        )
        .bind(event.id.to_string())
        .bind(event.ts.to_rfc3339())
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
                WHERE tenant_id = ? AND ts >= ?
                ORDER BY ts DESC
                LIMIT ?
                "#,
            )
            .bind(tenant.as_str())
            .bind(query.since.to_rfc3339())
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| FuseboxError::Storage(e.to_string()))?,
            None => sqlx::query(
                r#"
                SELECT id, ts, tenant_id, transition, reason
                FROM breaker_events
                WHERE ts >= ?
                ORDER BY ts DESC
                LIMIT ?
                "#,
            )
            .bind(query.since.to_rfc3339())
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| FuseboxError::Storage(e.to_string()))?,
        };

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let id_str: String = row
                .try_get("id")
                .map_err(|e| FuseboxError::Storage(e.to_string()))?;
            let ts_str: String = row
                .try_get("ts")
                .map_err(|e| FuseboxError::Storage(e.to_string()))?;
            let transition_str: String = row.try_get("transition").unwrap_or_default();
            let tenant_str: String = row.try_get("tenant_id").unwrap_or_default();
            let reason: Option<String> = row.try_get("reason").ok();
            out.push(BreakerEvent {
                id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
                ts: DateTime::parse_from_rfc3339(&ts_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                tenant_id: TenantId::from(tenant_str),
                transition: BreakerTransitionKind::from_str(&transition_str),
                reason,
            });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::SpendStatus;
    use chrono::Duration;
    use fusebox_core::TokenUsage;

    fn sample(tenant: &str, cost: f64) -> SpendEvent {
        SpendEvent::now(
            TenantId::from(tenant),
            Provider::OpenAI,
            ModelId::new("gpt-4o-mini"),
            TokenUsage {
                input_tokens: 100,
                output_tokens: 50,
                ..Default::default()
            },
            CostUsd(cost),
            SpendStatus::Completed,
        )
    }

    #[tokio::test]
    async fn round_trips_through_sqlite() {
        let ledger = SqliteLedger::in_memory().await.unwrap();
        ledger.record(sample("alice", 0.25)).await.unwrap();
        ledger.record(sample("alice", 0.10)).await.unwrap();
        ledger.record(sample("bob", 1.00)).await.unwrap();

        let totals = ledger
            .totals(&SpendQuery::for_tenant_since(
                TenantId::from("alice"),
                Utc::now() - Duration::hours(1),
            ))
            .await
            .unwrap();
        assert_eq!(totals.events, 2);
        assert!((totals.cost.0 - 0.35).abs() < 1e-6);

        let list = ledger
            .list(&SpendQuery::for_tenant_since(
                TenantId::from("alice"),
                Utc::now() - Duration::hours(1),
            ))
            .await
            .unwrap();
        assert_eq!(list.len(), 2);
    }
}
