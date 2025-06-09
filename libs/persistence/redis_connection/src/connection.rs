use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use deadpool_redis::{Connection, Pool, PoolError};

static REDIS_POOL: OnceLock<Pool> = OnceLock::new();

#[async_trait]
pub trait RedisConnect {
    type Connection: deadpool_redis::redis::aio::ConnectionLike + Send + Sync;
    async fn get_connection(&self) -> Result<Connection, PoolError>;
}

#[derive(Clone)]
pub struct RedisConnectionManager {
    pool: Pool,
}

impl RedisConnectionManager {
    pub fn new(pool: Pool) -> Self { Self { pool } }

    pub fn from_static() -> Self {
        let pool = REDIS_POOL
            .get()
            .expect("Redis pool not initialized")
            .clone();
        Self::new(pool)
    }

    pub fn init_static(pool: Pool) { REDIS_POOL.set(pool).ok(); }

    pub async fn get_connection(&self) -> Result<Connection, PoolError> {
        self.pool.get().await
    }
}

#[async_trait]
impl RedisConnect for RedisConnectionManager {
    type Connection = Connection;

    async fn get_connection(&self) -> Result<Connection, PoolError> {
        self.pool.get().await
    }
}

#[async_trait]
impl<T> RedisConnect for Arc<T>
where
    T: RedisConnect + Send + Sync,
{
    type Connection = Connection;

    async fn get_connection(&self) -> Result<Self::Connection, PoolError> {
        (**self).get_connection().await
    }
}
