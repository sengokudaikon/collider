use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum EventError {
    #[error("Database error: {0}")]
    Database(#[from] sql_connection::PgError),
    #[error("Connection error: {0}")]
    Connection(#[from] sql_connection::PoolError),
    #[error("Event type error: {0}")]
    EventType(#[from] EventTypeError),
    #[error("Event not found: {event_id}")]
    NotFound { event_id: Uuid },
    #[error("Redis error: {0}")]
    Redis(#[from] redis_connection::RedisError),
    #[error("Redis pool error: {0}")]
    Pool(#[from] redis_connection::PoolError),
}

#[derive(Debug, Error)]
pub enum EventTypeError {
    #[error("Database error: {0}")]
    Database(#[from] sql_connection::PgError),
    #[error("Connection error: {0}")]
    Connection(#[from] sql_connection::PoolError),
    #[error("Event type not found")]
    NotFound,
    #[error("Event type with this name already exists")]
    AlreadyExists,
}
