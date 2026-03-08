use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::Duration;

use opentelemetry::trace::{SpanContext, SpanId, TraceFlags, TraceId, TraceState};
use rskafka::client::ClientBuilder;
use rskafka::client::partition::{PartitionClient, UnknownTopicHandling};
use rskafka::record::Record;
use sqlx::Postgres;
use sqlx::pool::PoolConnection;
use tokio_util::sync::CancellationToken;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::config::KafkaConfig;

use super::OutboxError;
use super::db;
use super::db::OutboxRow;

/// Map event_type to Kafka topic.
fn route_topic(event_type: &str) -> Result<&'static str, OutboxError> {
    match event_type {
        "auth_email" => Ok("email.auth"),
        other => Err(OutboxError::UnknownEventType(other.to_string())),
    }
}

/// Build W3C traceparent header from outbox row trace context.
///
/// Format: `00-{trace_id}-{span_id}-01`
/// Returns `None` if trace context is missing or invalid.
fn build_traceparent(event: &OutboxRow) -> Option<String> {
    match (&event.trace_id, &event.span_id) {
        (Some(tid), Some(sid)) if !tid.is_empty() && !sid.is_empty() => {
            Some(format!("00-{tid}-{sid}-01"))
        }
        _ => None,
    }
}

/// Build an OpenTelemetry `SpanContext` from outbox row trace context for span linking.
fn build_span_context(event: &OutboxRow) -> Option<SpanContext> {
    let trace_id = TraceId::from_hex(event.trace_id.as_deref()?).ok()?;
    let span_id = SpanId::from_hex(event.span_id.as_deref()?).ok()?;
    Some(SpanContext::new(
        trace_id,
        span_id,
        TraceFlags::SAMPLED,
        true,
        TraceState::NONE,
    ))
}

/// Run the outbox publisher loop.
///
/// Acquires a PostgreSQL advisory lock for leader election, then polls the
/// outbox table for unpublished events and produces them to Kafka. Only one
/// replica will actively publish at a time.
pub async fn run(
    pool: sqlx::PgPool,
    config: &KafkaConfig,
    cancel: CancellationToken,
) -> Result<(), OutboxError> {
    let poll_interval = Duration::from_millis(config.poll_interval_ms);
    let batch_size = config.batch_size;

    let brokers: Vec<String> = config
        .brokers
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();

    // Connect to Kafka
    let kafka_client = ClientBuilder::new(brokers)
        .build()
        .await
        .map_err(|e| OutboxError::Kafka(e.to_string()))?;
    let kafka_client = Arc::new(kafka_client);

    // Cache partition clients per topic
    let mut partition_clients: HashMap<&'static str, Arc<PartitionClient>> = HashMap::new();

    // Acquire a dedicated (non-pooled) connection for the advisory lock.
    // Session-level advisory locks auto-release when this connection drops.
    let mut leader_conn: PoolConnection<Postgres> =
        pool.acquire().await.map_err(OutboxError::Db)?;

    let mut is_leader = false;

    tracing::info!("outbox publisher started");

    loop {
        tokio::select! {
            () = cancel.cancelled() => {
                tracing::info!("outbox publisher shutting down");
                break;
            }
            () = tokio::time::sleep(poll_interval) => {
                // Attempt leadership
                match db::try_acquire_leader_lock(&mut leader_conn).await {
                    Ok(true) => {
                        if !is_leader {
                            tracing::info!("outbox publisher acquired leader lock");
                            is_leader = true;
                        }
                    }
                    Ok(false) => {
                        if is_leader {
                            tracing::info!("outbox publisher lost leader lock");
                            is_leader = false;
                        }
                        continue;
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "outbox publisher leader lock error");
                        is_leader = false;
                        // Reconnect on next iteration
                        match pool.acquire().await {
                            Ok(conn) => leader_conn = conn,
                            Err(e) => tracing::error!(error = %e, "failed to reacquire connection"),
                        }
                        continue;
                    }
                }

                // Poll for unpublished events
                let events = match db::fetch_unpublished(&pool, batch_size).await {
                    Ok(events) => events,
                    Err(e) => {
                        tracing::error!(error = %e, "failed to fetch unpublished events");
                        continue;
                    }
                };

                if events.is_empty() {
                    continue;
                }

                tracing::debug!(count = events.len(), "processing outbox events");

                for event in events {
                    let topic = match route_topic(&event.event_type) {
                        Ok(t) => t,
                        Err(e) => {
                            tracing::error!(
                                event_id = %event.id,
                                event_type = %event.event_type,
                                error = %e,
                                "unknown event type, skipping"
                            );
                            continue;
                        }
                    };

                    // Create a linked span from the originating trace context
                    let publish_span = tracing::info_span!(
                        "outbox.publish",
                        event_id = %event.id,
                        event_type = %event.event_type,
                        topic,
                    );
                    if let Some(origin_ctx) = build_span_context(&event) {
                        publish_span.add_link(origin_ctx);
                    }
                    let _guard = publish_span.enter();

                    // Build Kafka headers with W3C trace propagation from the
                    // *current* publisher span (not the originating span) so
                    // downstream consumers continue the publisher's trace.
                    let headers = {
                        use opentelemetry::trace::TraceContextExt;
                        let ctx = publish_span.context();
                        let sc = ctx.span().span_context().clone();
                        let mut h = BTreeMap::new();
                        if sc.is_valid() {
                            let traceparent = format!(
                                "00-{}-{}-{:02x}",
                                sc.trace_id(),
                                sc.span_id(),
                                sc.trace_flags().to_u8(),
                            );
                            h.insert("traceparent".to_string(), traceparent.into_bytes());
                        } else if let Some(tp) = build_traceparent(&event) {
                            // Fallback: propagate original trace context if publisher
                            // has no active OTEL span (e.g. in tests).
                            h.insert("traceparent".to_string(), tp.into_bytes());
                        }
                        h
                    };

                    // Get or create partition client for this topic
                    let partition_client = match partition_clients.get(topic) {
                        Some(pc) => Arc::clone(pc),
                        None => {
                            match kafka_client
                                .partition_client(topic, 0, UnknownTopicHandling::Retry)
                                .await
                            {
                                Ok(pc) => {
                                    let pc = Arc::new(pc);
                                    partition_clients.insert(topic, Arc::clone(&pc));
                                    pc
                                }
                                Err(e) => {
                                    tracing::error!(
                                        topic,
                                        error = %e,
                                        "failed to create partition client"
                                    );
                                    break;
                                }
                            }
                        }
                    };

                    // Produce the record
                    let record = Record {
                        key: Some(event.aggregate_id.to_string().into_bytes()),
                        value: Some(event.payload),
                        headers,
                        timestamp: chrono::Utc::now(),
                    };

                    if let Err(e) =
                        partition_client
                            .produce(vec![record], Default::default())
                            .await
                    {
                        tracing::error!(
                            event_id = %event.id,
                            topic,
                            error = %e,
                            "failed to produce to kafka, will retry"
                        );
                        // Don't mark published — will be retried next poll
                        break;
                    }

                    // Mark as published
                    if let Err(e) = db::mark_published(&pool, event.id).await {
                        tracing::error!(
                            event_id = %event.id,
                            error = %e,
                            "failed to mark event published (duplicate possible)"
                        );
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_auth_email() {
        assert_eq!(route_topic("auth_email").unwrap(), "email.auth");
    }

    #[test]
    fn route_unknown_event_type() {
        let err = route_topic("unknown_event").unwrap_err();
        assert!(matches!(err, OutboxError::UnknownEventType(_)));
    }

    #[test]
    fn traceparent_with_valid_context() {
        let event = OutboxRow {
            id: uuid::Uuid::nil(),
            aggregate_type: String::new(),
            aggregate_id: uuid::Uuid::nil(),
            event_type: String::new(),
            payload: Vec::new(),
            trace_id: Some("0af7651916cd43dd8448eb211c80319c".into()),
            span_id: Some("b7ad6b7169203331".into()),
            created_at: chrono::Utc::now(),
        };
        assert_eq!(
            build_traceparent(&event).unwrap(),
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01"
        );
    }

    #[test]
    fn traceparent_missing_context() {
        let event = OutboxRow {
            id: uuid::Uuid::nil(),
            aggregate_type: String::new(),
            aggregate_id: uuid::Uuid::nil(),
            event_type: String::new(),
            payload: Vec::new(),
            trace_id: None,
            span_id: None,
            created_at: chrono::Utc::now(),
        };
        assert!(build_traceparent(&event).is_none());
    }

    #[test]
    fn span_context_from_valid_ids() {
        let event = OutboxRow {
            id: uuid::Uuid::nil(),
            aggregate_type: String::new(),
            aggregate_id: uuid::Uuid::nil(),
            event_type: String::new(),
            payload: Vec::new(),
            trace_id: Some("0af7651916cd43dd8448eb211c80319c".into()),
            span_id: Some("b7ad6b7169203331".into()),
            created_at: chrono::Utc::now(),
        };
        let ctx = build_span_context(&event).unwrap();
        assert_eq!(
            ctx.trace_id().to_string(),
            "0af7651916cd43dd8448eb211c80319c"
        );
        assert_eq!(ctx.span_id().to_string(), "b7ad6b7169203331");
    }

    #[test]
    fn span_context_from_invalid_ids() {
        let event = OutboxRow {
            id: uuid::Uuid::nil(),
            aggregate_type: String::new(),
            aggregate_id: uuid::Uuid::nil(),
            event_type: String::new(),
            payload: Vec::new(),
            trace_id: Some("not-hex".into()),
            span_id: Some("also-not-hex".into()),
            created_at: chrono::Utc::now(),
        };
        assert!(build_span_context(&event).is_none());
    }
}
