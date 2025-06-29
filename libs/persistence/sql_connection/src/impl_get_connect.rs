use std::convert::Infallible;

use database_traits::connection::{FromRequestParts, Parts};
use deadpool_postgres::{Object, Pool};
use tracing::{instrument, warn};

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
    #[instrument(skip(self), fields(pool_type = "primary"))]
    pub async fn get_client(
        &self,
    ) -> Result<Object, deadpool_postgres::PoolError> {
        let status = self.pool.status();
        if status.available == 0 {
            warn!(
                "Primary pool exhausted! Available: {}, Size: {}, Max: {}",
                status.available, status.size, status.max_size
            );
        }

        match self.pool.get().await {
            Ok(conn) => {
                let new_status = self.pool.status();
                if new_status.available < 10 {
                    warn!(
                        "Primary pool running low! Available: {}, Size: {}",
                        new_status.available, new_status.size
                    );
                }
                Ok(conn)
            }
            Err(e) => {
                warn!("Failed to get primary connection: {}", e);
                Err(e)
            }
        }
    }

    /// Get connection for read operations (uses read replica if available)
    #[instrument(skip(self), fields(pool_type = if self.enable_read_write_split { "read_replica" } else { "primary" }))]
    pub async fn get_read_client(
        &self,
    ) -> Result<Object, deadpool_postgres::PoolError> {
        if let Some(read_pool) = &self.read_pool {
            if self.enable_read_write_split {
                let status = read_pool.status();
                if status.available == 0 {
                    warn!(
                        "Read replica pool exhausted! Available: {}, Size: \
                         {}, Max: {}",
                        status.available, status.size, status.max_size
                    );
                }

                match read_pool.get().await {
                    Ok(conn) => {
                        let new_status = read_pool.status();
                        if new_status.available < 20 {
                            warn!(
                                "Read replica pool running low! Available: \
                                 {}, Size: {}",
                                new_status.available, new_status.size
                            );
                        }
                        return Ok(conn);
                    }
                    Err(e) => {
                        warn!(
                            "Failed to get read replica connection: {}, \
                             falling back to primary",
                            e
                        );
                        // Fall through to use primary pool
                    }
                }
            }
        }
        // Fallback to primary pool if no read replica
        self.get_client().await
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
