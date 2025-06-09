use std::{
    sync::atomic::{AtomicU32, Ordering},
    time::Duration,
};

use deadpool_redis::{Config, Pool, Runtime};
use redis_connection::connection::RedisConnectionManager;
use tokio::time::sleep;

static REDIS_DB_COUNTER: AtomicU32 = AtomicU32::new(1);

pub struct TestRedisContainer {
    pub pool: Pool,
    pub connection_string: String,
}

impl TestRedisContainer {
    pub async fn new() -> anyhow::Result<Self> {
        let db_number = REDIS_DB_COUNTER.fetch_add(1, Ordering::SeqCst);
        let connection_string =
            format!("redis://localhost:6380/{}", db_number);
        Self::new_with_connection_string(&connection_string).await
    }

    pub async fn new_with_connection_string(
        connection_string: &str,
    ) -> anyhow::Result<Self> {
        let connection_string = connection_string.to_string();

        sleep(Duration::from_secs(2)).await;

        let mut cfg = Config::from_url(&connection_string);
        cfg.pool = Some(deadpool_redis::PoolConfig::new(10));
        let pool = cfg.create_pool(Some(Runtime::Tokio1))?;

        let mut attempts = 0;
        loop {
            match pool.get().await {
                Ok(mut conn) => {
                    match deadpool_redis::redis::cmd("PING")
                        .query_async::<()>(&mut conn)
                        .await
                    {
                        Ok(_) => break,
                        Err(_) if attempts < 10 => {
                            attempts += 1;
                            sleep(Duration::from_millis(500 * attempts))
                                .await;
                            continue;
                        }
                        Err(e) => return Err(e.into()),
                    }
                }
                Err(_) if attempts < 10 => {
                    attempts += 1;
                    sleep(Duration::from_millis(500 * attempts)).await;
                    continue;
                }
                Err(e) => return Err(e.into()),
            }
        }

        // Initialize the static Redis pool for tests
        RedisConnectionManager::init_static(pool.clone());

        Ok(Self {
            pool,
            connection_string,
        })
    }

    pub async fn get_connection(
        &self,
    ) -> anyhow::Result<deadpool_redis::Connection> {
        Ok(self.pool.get().await?)
    }

    pub async fn flush_db(&self) -> anyhow::Result<()> {
        let mut conn = self.get_connection().await?;
        deadpool_redis::redis::cmd("FLUSHDB")
            .query_async::<()>(&mut conn)
            .await?;
        Ok(())
    }
}
