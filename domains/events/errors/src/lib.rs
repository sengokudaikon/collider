use common_errors::AppError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EventError {
    #[error("Database error: {0}")]
    Database(#[from] sql_connection::PgError),
    #[error("Connection error: {0}")]
    Connection(#[from] sql_connection::PoolError),
    #[error("Event type error: {0}")]
    EventType(#[from] EventTypeError),
    #[error("Event not found: {event_id}")]
    NotFound { event_id: i64 },
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

impl From<EventError> for AppError {
    fn from(err: EventError) -> Self {
        match err {
            EventError::NotFound { event_id } => {
                AppError::not_found(
                    "EVENT_NOT_FOUND",
                    &format!("Event with ID {} not found", event_id),
                )
            }
            EventError::EventType(event_type_err) => {
                match event_type_err {
                    EventTypeError::NotFound => {
                        AppError::not_found(
                            "EVENT_TYPE_NOT_FOUND",
                            "Event type not found",
                        )
                    }
                    EventTypeError::AlreadyExists => {
                        AppError::unprocessable_entity(
                            "EVENT_TYPE_EXISTS",
                            "An event type with this name already exists",
                        )
                    }
                    EventTypeError::Database(db_err) => {
                        AppError::internal_server_error(&format!(
                            "Database error: {}",
                            db_err
                        ))
                    }
                    EventTypeError::Connection(conn_err) => {
                        AppError::internal_server_error(&format!(
                            "Database connection error: {}",
                            conn_err
                        ))
                    }
                }
            }
            EventError::Database(db_err) => {
                AppError::internal_server_error(&format!(
                    "Database error: {}",
                    db_err
                ))
            }
            EventError::Connection(conn_err) => {
                AppError::internal_server_error(&format!(
                    "Database connection error: {}",
                    conn_err
                ))
            }
            EventError::Redis(redis_err) => {
                AppError::internal_server_error(&format!(
                    "Cache error: {}",
                    redis_err
                ))
            }
            EventError::Pool(pool_err) => {
                AppError::internal_server_error(&format!(
                    "Cache connection error: {}",
                    pool_err
                ))
            }
        }
    }
}
