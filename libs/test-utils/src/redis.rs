use std::{
    sync::atomic::{AtomicU32, Ordering},
    time::Duration,
};

use deadpool_redis::{Config, Pool, Runtime};
use redis_connection::connection::RedisConnectionManager;
use tokio::time::sleep;

static TEST_INSTANCE_COUNTER: AtomicU32 = AtomicU32::new(1);

pub struct TestRedisContainer {
    pub pool: Pool,
    pub connection_string: String,
    pub test_prefix: String,
}

impl TestRedisContainer {
    pub async fn new() -> anyhow::Result<Self> {
        let test_instance =
            TEST_INSTANCE_COUNTER.fetch_add(1, Ordering::SeqCst);
        // Use database 0 (compatible with Dragonfly) and rely on key prefixes
        // for isolation
        let connection_string = "redis://localhost:6379/0".to_string();
        let test_prefix = format!("test_{}:", test_instance);
        Self::new_with_connection_string_and_prefix(
            &connection_string,
            &test_prefix,
        )
        .await
    }

    pub async fn new_with_connection_string(
        connection_string: &str,
    ) -> anyhow::Result<Self> {
        let test_instance =
            TEST_INSTANCE_COUNTER.fetch_add(1, Ordering::SeqCst);
        let test_prefix = format!("test_{}:", test_instance);
        Self::new_with_connection_string_and_prefix(
            connection_string,
            &test_prefix,
        )
        .await
    }

    pub async fn new_with_connection_string_and_prefix(
        connection_string: &str, test_prefix: &str,
    ) -> anyhow::Result<Self> {
        let connection_string = connection_string.to_string();
        let test_prefix = test_prefix.to_string();

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
            test_prefix,
        })
    }

    pub async fn get_connection(
        &self,
    ) -> anyhow::Result<deadpool_redis::Connection> {
        Ok(self.pool.get().await?)
    }

    pub async fn flush_db(&self) -> anyhow::Result<()> {
        let mut conn = self.get_connection().await?;

        // Get all keys with this test's prefix and delete them
        let pattern = format!("{}*", self.test_prefix);
        let keys: Vec<String> = deadpool_redis::redis::cmd("KEYS")
            .arg(&pattern)
            .query_async(&mut conn)
            .await?;

        if !keys.is_empty() {
            deadpool_redis::redis::cmd("DEL")
                .arg(&keys)
                .query_async::<()>(&mut conn)
                .await?;
        }

        Ok(())
    }

    /// Get a test-prefixed key for isolation
    pub fn test_key(&self, key: &str) -> String {
        format!("{}{}", self.test_prefix, key)
    }
}
