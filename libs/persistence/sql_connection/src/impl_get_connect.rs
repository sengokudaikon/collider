use std::convert::Infallible;

use database_traits::connection::{FromRequestParts, Parts};
use deadpool_postgres::{Object, Pool};

use crate::static_vars::{get_read_sql_pool, get_sql_pool};

#[derive(Debug, Clone)]
pub struct SqlConnect {
    pool: Pool,
    read_pool: Option<Pool>,
    enable_read_write_split: bool,
}

impl SqlConnect {
    pub fn new(pool: Pool) -> Self {
        Self {
            pool,
            read_pool: None,
            enable_read_write_split: false,
        }
    }

    pub fn new_with_read_replica(pool: Pool, read_pool: Pool) -> Self {
        Self {
            pool,
            read_pool: Some(read_pool),
            enable_read_write_split: true,
        }
    }

    pub fn from_global() -> Self {
        let read_pool = get_read_sql_pool().cloned();
        let enable_read_write_split = read_pool.is_some();

        Self {
            pool: get_sql_pool().clone(),
            read_pool,
            enable_read_write_split,
        }
    }

    /// Get connection for write operations (always uses primary database)
    pub async fn get_client(
        &self,
    ) -> Result<Object, deadpool_postgres::PoolError> {
        self.pool.get().await
    }

    /// Get connection for read operations (uses read replica if available)
    pub async fn get_read_client(
        &self,
    ) -> Result<Object, deadpool_postgres::PoolError> {
        if let Some(read_pool) = &self.read_pool {
            if self.enable_read_write_split {
                return read_pool.get().await;
            }
        }
        // Fallback to primary pool if no read replica
        self.pool.get().await
    }

    /// Get connection optimized for heavy analytics queries
    pub async fn get_analytics_client(
        &self,
    ) -> Result<Object, deadpool_postgres::PoolError> {
        // Always prefer read replica for analytics to avoid impacting writes
        self.get_read_client().await
    }

    /// Check if read-write splitting is enabled
    pub fn has_read_replica(&self) -> bool {
        self.enable_read_write_split && self.read_pool.is_some()
    }

    /// Get pool statistics for monitoring
    pub fn get_pool_status(&self) -> (usize, usize, Option<(usize, usize)>) {
        let write_pool = &self.pool;
        let write_stats =
            (write_pool.status().available, write_pool.status().size);

        let read_stats = self
            .read_pool
            .as_ref()
            .map(|pool| (pool.status().available, pool.status().size));

        (write_stats.0, write_stats.1, read_stats)
    }
}

impl Default for SqlConnect {
    fn default() -> Self { Self::from_global() }
}

impl<S> FromRequestParts<S> for SqlConnect {
    type Rejection = Infallible;

    fn from_request_parts(
        _parts: &mut Parts, _state: &S,
    ) -> impl std::future::Future<
        Output = Result<Self, <Self as FromRequestParts<S>>::Rejection>,
    > + Send {
        Box::pin(async { Ok(SqlConnect::from_global()) })
    }
}
