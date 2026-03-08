#[derive(Debug, thiserror::Error)]
pub enum OutboxError {
    #[error("database error")]
    Db(#[from] sqlx::Error),

    #[error("kafka error: {0}")]
    Kafka(String),

    #[error("unknown event type: {0}")]
    UnknownEventType(String),
}
