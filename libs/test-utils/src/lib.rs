pub mod sql_migrator;
pub mod test_helpers;

use std::time::Duration;

use anyhow::{Context, Result};
use deadpool_postgres::{
    Manager, ManagerConfig, Pool as PostgresPool, RecyclingMethod,
};
use deadpool_redis::{Config as RedisConfig, Pool as RedisPool, Runtime};
use redis_connection::connection::RedisConnectionManager;
pub use test_helpers::*;
use testcontainers_modules::{
    postgres::Postgres,
    redis::Redis,
    testcontainers::{ImageExt, runners::AsyncRunner},
};
use tokio_postgres::NoTls;

pub use crate::sql_migrator::SqlMigrator;

/// Modern PostgreSQL test container using testcontainers-rs
pub struct TestPostgresContainer {
    pub pool: PostgresPool,
    pub connection_string: String,
    // Keep the container alive for the lifetime of this struct
    _container:
        testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
}

impl TestPostgresContainer {
    /// Create a new PostgreSQL test container
    ///
    /// This will:
    /// 1. Start a fresh PostgreSQL container with a random port
    /// 2. Create a connection pool
    /// 3. Run database migrations
    /// 4. Return a ready-to-use container
    pub async fn new() -> Result<Self> {
        // Start a PostgreSQL container
        let container = Postgres::default()
            .with_env_var("POSTGRES_DB", "testdb")
            .with_env_var("POSTGRES_USER", "testuser")
            .with_env_var("POSTGRES_PASSWORD", "testpass")
            .start()
            .await
            .context("Failed to start PostgreSQL container")?;

        // Get connection details
        let host = container.get_host().await?;
        let port = container.get_host_port_ipv4(5432).await?;
        let connection_string = format!(
            "postgresql://testuser:testpass@{host}:{port}/testdb"
        );

        // Wait for PostgreSQL to be ready and create connection pool
        let pool = Self::create_pool(&connection_string).await?;

        let instance = Self {
            pool,
            connection_string,
            _container: container,
        };

        // Apply migrations
        instance.apply_migrations().await?;

        Ok(instance)
    }

    async fn create_pool(connection_string: &str) -> Result<PostgresPool> {
        let pg_config =
            connection_string.parse::<tokio_postgres::Config>()?;

        let mgr_config = ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        };
        let mgr = Manager::from_config(pg_config, NoTls, mgr_config);

        let pool = PostgresPool::builder(mgr)
            .max_size(10)
            .build()
            .context("Failed to build PostgreSQL connection pool")?;

        // Test the connection
        let mut attempts = 0;
        loop {
            match pool.get().await {
                Ok(client) => {
                    match client.query_one("SELECT 1", &[]).await {
                        Ok(_) => break,
                        Err(_) if attempts < 20 => {
                            attempts += 1;
                            tokio::time::sleep(Duration::from_millis(500))
                                .await;
                            continue;
                        }
                        Err(e) => {
                            return Err(e).context("PostgreSQL not ready");
                        }
                    }
                }
                Err(_) if attempts < 20 => {
                    attempts += 1;
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    continue;
                }
                Err(e) => {
                    return Err(e)
                        .context("Failed to get PostgreSQL connection");
                }
            }
        }

        Ok(pool)
    }

    pub async fn execute_sql(&self, sql: &str) -> Result<()> {
        let client = self.pool.get().await?;
        client
            .execute(sql, &[])
            .await
            .context("Failed to execute SQL")?;
        Ok(())
    }

    async fn apply_migrations(&self) -> Result<()> {
        let migrator = self.get_migrator().await?;
        migrator
            .run_all_migrations()
            .await
            .context("Failed to apply migrations")
    }

    pub async fn get_migrator(&self) -> Result<SqlMigrator> {
        Ok(SqlMigrator::new(self.pool.clone()))
    }
}

/// Modern Redis test container using testcontainers-rs
pub struct TestRedisContainer {
    pub pool: RedisPool,
    pub connection_string: String,
    pub test_prefix: String,
    // Keep the container alive for the lifetime of this struct
    _container: testcontainers_modules::testcontainers::ContainerAsync<Redis>,
}

impl TestRedisContainer {
    /// Create a new Redis test container
    ///
    /// This will:
    /// 1. Start a fresh Redis container with a random port
    /// 2. Create a connection pool
    /// 3. Set up key prefixing for test isolation
    /// 4. Return a ready-to-use container
    pub async fn new() -> Result<Self> {
        // Start a Redis container
        let container = Redis::default()
            .start()
            .await
            .context("Failed to start Redis container")?;

        // Get connection details
        let host = container.get_host().await?;
        let port = container.get_host_port_ipv4(6379).await?;
        let connection_string = format!("redis://{host}:{port}");

        // Create unique test prefix
        let test_prefix = format!("test_{}:", uuid::Uuid::now_v7().simple());

        // Create connection pool
        let pool = Self::create_pool(&connection_string).await?;

        // Initialize the static Redis pool for tests
        RedisConnectionManager::init_static(pool.clone());

        Ok(Self {
            pool,
            connection_string,
            test_prefix,
            _container: container,
        })
    }

    async fn create_pool(connection_string: &str) -> Result<RedisPool> {
        let mut cfg = RedisConfig::from_url(connection_string);
        cfg.pool = Some(deadpool_redis::PoolConfig::new(10));
        let pool = cfg
            .create_pool(Some(Runtime::Tokio1))
            .context("Failed to create Redis pool")?;

        // Test the connection
        let mut attempts = 0;
        loop {
            match pool.get().await {
                Ok(mut conn) => {
                    match deadpool_redis::redis::cmd("PING")
                        .query_async::<()>(&mut conn)
                        .await
                    {
                        Ok(_) => break,
                        Err(_) if attempts < 20 => {
                            attempts += 1;
                            tokio::time::sleep(Duration::from_millis(500))
                                .await;
                            continue;
                        }
                        Err(e) => return Err(e).context("Redis not ready"),
                    }
                }
                Err(_) if attempts < 20 => {
                    attempts += 1;
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    continue;
                }
                Err(e) => {
                    return Err(e).context("Failed to get Redis connection");
                }
            }
        }

        Ok(pool)
    }

    pub async fn get_connection(&self) -> Result<deadpool_redis::Connection> {
        Ok(self.pool.get().await?)
    }

    pub async fn flush_db(&self) -> Result<()> {
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

    pub async fn flush_all_keys(&self) -> anyhow::Result<()> {
        let mut conn = self.get_connection().await?;

        // Get all keys and delete them
        let keys: Vec<String> = deadpool_redis::redis::cmd("KEYS")
            .arg("*")
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

// Configuration support for modern containers
#[derive(serde::Deserialize)]
pub struct ModernTestDbConfig {
    pub connection_string: String,
}

impl sql_connection::DbConnectConfig for ModernTestDbConfig {
    fn scheme(&self) -> &str { "postgresql" }

    fn username(&self) -> &str { "" }

    fn password(&self) -> &str { "" }

    fn host(&self) -> &str { "" }

    fn port(&self) -> u16 { 5432 }

    fn name(&self) -> &str { "" }

    fn uri(&self) -> &str { &self.connection_string }
}

impl sql_connection::DbOptionsConfig for ModernTestDbConfig {
    fn max_conn(&self) -> Option<u32> { Some(10) }

    fn min_conn(&self) -> Option<u32> { Some(2) }

    fn sql_logger(&self) -> bool { false }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_modern_postgres_container() {
        let container = TestPostgresContainer::new().await.unwrap();

        // Test that we can execute SQL
        container.execute_sql("SELECT 1").await.unwrap();

        // Test that we can get a connection
        let client = container.pool.get().await.unwrap();
        let result: i32 =
            client.query_one("SELECT 1", &[]).await.unwrap().get(0);
        assert_eq!(result, 1);
    }

    #[tokio::test]
    async fn test_modern_redis_container() {
        let container = TestRedisContainer::new().await.unwrap();

        // Test that we can get a connection
        let mut conn = container.get_connection().await.unwrap();

        // Test basic Redis operations
        let _: () = deadpool_redis::redis::cmd("SET")
            .arg(container.test_key("test_key"))
            .arg("test_value")
            .query_async(&mut conn)
            .await
            .unwrap();

        let value: String = deadpool_redis::redis::cmd("GET")
            .arg(container.test_key("test_key"))
            .query_async(&mut conn)
            .await
            .unwrap();

        assert_eq!(value, "test_value");
    }

    #[tokio::test]
    async fn test_multiple_postgres_containers_isolated() {
        let container1 = TestPostgresContainer::new().await.unwrap();
        let container2 = TestPostgresContainer::new().await.unwrap();

        // Containers should have different connection strings (different
        // ports)
        assert_ne!(
            container1.connection_string,
            container2.connection_string
        );

        // Both should work independently
        container1
            .execute_sql("CREATE TABLE test1 (id INT)")
            .await
            .unwrap();
        container2
            .execute_sql("CREATE TABLE test2 (id INT)")
            .await
            .unwrap();

        // Test that tables don't interfere with each other
        let client1 = container1.pool.get().await.unwrap();
        let client2 = container2.pool.get().await.unwrap();

        // Table exists in container1 but not container2
        assert!(client1.query("SELECT * FROM test1", &[]).await.is_ok());
        assert!(client2.query("SELECT * FROM test1", &[]).await.is_err());

        // Table exists in container2 but not container1
        assert!(client2.query("SELECT * FROM test2", &[]).await.is_ok());
        assert!(client1.query("SELECT * FROM test2", &[]).await.is_err());
    }
}
