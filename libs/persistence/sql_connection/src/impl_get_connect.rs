use std::convert::Infallible;

use database_traits::connection::{FromRequestParts, Parts};
use deadpool_postgres::{Object, Pool};
use tracing::{instrument, warn};

use crate::static_vars::get_sql_pool;

#[derive(Debug, Clone)]
pub struct SqlConnect {
    pool: Pool, /* Single pool for BRRRRR mode - all 1000 connections on
                 * primary */
}

impl SqlConnect {
    pub fn new(pool: Pool) -> Self { Self { pool } }

    pub fn from_global() -> Self {
        Self {
            pool: get_sql_pool().clone(),
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

    /// Get connection for read operations (uses same pool as writes in BRRRRR
    /// mode)
    #[instrument(skip(self), fields(pool_type = "primary"))]
    pub async fn get_read_client(
        &self,
    ) -> Result<Object, deadpool_postgres::PoolError> {
        // In BRRRRR mode, all reads and writes use the same 1000-connection
        // pool
        self.get_client().await
    }

    /// Get connection optimized for heavy analytics queries
    pub async fn get_analytics_client(
        &self,
    ) -> Result<Object, deadpool_postgres::PoolError> {
        // In BRRRRR mode, analytics use the same pool as everything else
        self.get_client().await
    }

    /// Check if read-write splitting is enabled (always false in BRRRRR mode)
    pub fn has_read_replica(&self) -> bool {
        false // Read replica removed for BRRRRR mode
    }

    /// Get pool statistics for monitoring (only primary pool in BRRRRR mode)
    pub fn get_pool_status(&self) -> (usize, usize, Option<(usize, usize)>) {
        let pool_status = self.pool.status();
        (pool_status.available, pool_status.size, None) // No read replica stats
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
