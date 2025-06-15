use redis_connection::{PoolError, RedisError};
use sql_connection::{PgError, PoolError as DbPoolError};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum UserError {
    #[error("User not found: {user_id}")]
    NotFound { user_id: Uuid },
    #[error("User not found: {username}")]
    NameNotFound { username: String },
    #[error("Database error: {0}")]
    Database(#[from] PgError),
    #[error("Database Pool error: {0}")]
    DatabasePool(#[from] DbPoolError),
    #[error("Redis error: {0}")]
    Redis(#[from] RedisError),
    #[error("Redis Pool error: {0}")]
    RedisPool(#[from] PoolError),
    #[error("Name already exists")]
    NameExists,
}
