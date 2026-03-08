use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::OutboxError;

/// Advisory lock ID for leader election.
/// Hash of "outbox_publisher" truncated to i64.
const OUTBOX_PUBLISHER_LOCK_ID: i64 = 0x6F75_7462_6F78_7075;

/// A row from the outbox table.
pub struct OutboxRow {
    pub id: Uuid,
    pub aggregate_type: String,
    pub aggregate_id: Uuid,
    pub event_type: String,
    pub payload: Vec<u8>,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Insert an outbox event within an existing transaction.
///
/// The caller is responsible for committing the transaction, ensuring the
/// outbox event is written atomically with the domain operation.
#[allow(clippy::too_many_arguments)]
pub async fn insert_event(
    conn: &mut PgConnection,
    id: Uuid,
    aggregate_type: &str,
    aggregate_id: Uuid,
    event_type: &str,
    payload: &[u8],
    trace_id: Option<&str>,
    span_id: Option<&str>,
) -> Result<(), OutboxError> {
    sqlx::query!(
        "INSERT INTO outbox (id, aggregate_type, aggregate_id, event_type, payload, trace_id, span_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
        id,
        aggregate_type,
        aggregate_id,
        event_type,
        payload,
        trace_id,
        span_id,
    )
    .execute(conn)
    .await?;

    Ok(())
}

/// Attempt to acquire the outbox publisher advisory lock.
///
/// Uses a session-level advisory lock so it auto-releases when the connection
/// drops. Must be called on a **dedicated** (non-pooled) connection.
/// Returns `true` if this connection now holds the lock.
pub async fn try_acquire_leader_lock(conn: &mut PgConnection) -> Result<bool, OutboxError> {
    let row = sqlx::query_scalar!("SELECT pg_try_advisory_lock($1)", OUTBOX_PUBLISHER_LOCK_ID,)
        .fetch_one(conn)
        .await?;

    Ok(row.unwrap_or(false))
}

/// Fetch unpublished outbox events ordered by creation time.
pub async fn fetch_unpublished(
    pool: &PgPool,
    batch_size: i64,
) -> Result<Vec<OutboxRow>, OutboxError> {
    let rows = sqlx::query_as!(
        OutboxRow,
        r#"SELECT id, aggregate_type, aggregate_id, event_type, payload,
                  trace_id, span_id, created_at
           FROM outbox
           WHERE published_at IS NULL
           ORDER BY created_at ASC
           LIMIT $1"#,
        batch_size,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Mark an outbox event as published.
pub async fn mark_published(pool: &PgPool, id: Uuid) -> Result<(), OutboxError> {
    sqlx::query!("UPDATE outbox SET published_at = NOW() WHERE id = $1", id,)
        .execute(pool)
        .await?;

    Ok(())
}
