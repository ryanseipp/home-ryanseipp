#[derive(Debug, thiserror::Error)]
pub enum OutboxError {
    #[error("database error")]
    Db(#[from] tokio_postgres::Error),

    #[error("pool error")]
    Pool(#[from] deadpool_postgres::PoolError),

    #[error("kafka error: {0}")]
    Kafka(String),

    #[error("unknown event type: {0}")]
    UnknownEventType(String),
}
