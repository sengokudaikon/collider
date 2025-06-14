use thiserror::Error;

#[derive(Debug, Error)]
pub enum CommonDaoError {
    #[error("Database error: {0}")]
    Database(#[from] tokio_postgres::Error),
    #[error("Connection error: {0}")]
    Connection(#[from] deadpool_postgres::PoolError),
    #[error("Record not found")]
    NotFound,
}

pub trait DaoErrorExt {
    fn not_found_if_empty<T>(self, rows: &[T]) -> Result<(), CommonDaoError>;
}

impl DaoErrorExt for Result<(), CommonDaoError> {
    fn not_found_if_empty<T>(self, rows: &[T]) -> Result<(), CommonDaoError> {
        if rows.is_empty() {
            Err(CommonDaoError::NotFound)
        } else {
            self
        }
    }
}