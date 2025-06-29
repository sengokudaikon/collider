use common_errors::AppError;
use redis_connection::{PoolError, RedisError};
use sql_connection::{PgError, PoolError as DbPoolError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UserError {
    #[error("User not found: {user_id}")]
    NotFound { user_id: i64 },
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
    #[error("Internal error: {0}")]
    InternalError(String),
}

impl From<UserError> for AppError {
    fn from(err: UserError) -> Self {
        match err {
            UserError::NotFound { user_id } => {
                AppError::not_found(
                    "USER_NOT_FOUND",
                    &format!("User with ID {user_id} not found"),
                )
            }
            UserError::NameNotFound { username } => {
                AppError::not_found(
                    "USER_NOT_FOUND",
                    &format!("User with name '{username}' not found"),
                )
            }
            UserError::NameExists => {
                AppError::unprocessable_entity(
                    "USER_NAME_EXISTS",
                    "A user with this name already exists",
                )
            }
            UserError::Database(db_err) => {
                AppError::internal_server_error(&format!(
                    "Database error: {db_err}"
                ))
            }
            UserError::DatabasePool(pool_err) => {
                AppError::internal_server_error(&format!(
                    "Database connection error: {pool_err}"
                ))
            }
            UserError::Redis(redis_err) => {
                AppError::internal_server_error(&format!(
                    "Cache error: {redis_err}"
                ))
            }
            UserError::RedisPool(pool_err) => {
                AppError::internal_server_error(&format!(
                    "Cache connection error: {pool_err}"
                ))
            }
            UserError::InternalError(msg) => {
                AppError::internal_server_error(&format!(
                    "Internal error: {msg}"
                ))
            }
        }
    }
}
