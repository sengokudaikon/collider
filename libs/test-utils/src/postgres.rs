use std::time::Duration;

use anyhow::{Context, Result};
use sea_orm::{Database, DatabaseConnection};
use sqlx::postgres::PgPoolOptions;
use testcontainers::{runners::AsyncRunner, ContainerAsync};
use testcontainers_modules::postgres::Postgres;
use tokio::time::sleep;

use crate::sql_migrator::SqlMigrator;

pub struct TestPostgresContainer {
    #[allow(dead_code)]
    container: ContainerAsync<Postgres>,
    pub connection: DatabaseConnection,
    pub connection_string: String,
}

impl TestPostgresContainer {
    /// Creates a new test container with migrations applied.
    /// Each test gets a completely fresh PostgreSQL instance with schema.
    pub async fn new() -> Result<Self> {
        let container = Postgres::default()
            .start()
            .await
            .context("Failed to start PostgreSQL container")?;

        let connection_string =
            Self::build_connection_string(&container).await?;

        // Wait for PostgreSQL to be ready and create test database
        Self::wait_for_postgres_and_setup(&container).await?;

        // Create SeaORM connection
        let connection = Database::connect(&connection_string)
            .await
            .context("Failed to create database connection")?;

        let instance = Self {
            container,
            connection,
            connection_string,
        };

        // Apply migrations
        instance.apply_migrations().await?;

        Ok(instance)
    }

    /// Executes raw SQL against the test database
    pub async fn execute_sql(&self, sql: &str) -> Result<()> {
        // Extract sqlx pool from SeaORM connection for raw SQL execution
        let sqlx_pool = self.connection.get_postgres_connection_pool();
        sqlx::query(sql)
            .execute(sqlx_pool)
            .await
            .context("Failed to execute SQL")?;
        Ok(())
    }

    /// Applies all migrations to the test database
    async fn apply_migrations(&self) -> Result<()> {
        let sqlx_pool = self.connection.get_postgres_connection_pool();
        let migrator = SqlMigrator::new(sqlx_pool.clone());
        migrator
            .run_all_migrations()
            .await
            .context("Failed to apply migrations")
    }

    /// Builds the connection string for the test database
    async fn build_connection_string(
        container: &ContainerAsync<Postgres>,
    ) -> Result<String> {
        let host = container
            .get_host()
            .await
            .context("Failed to get container host")?;
        let port = container
            .get_host_port_ipv4(5432)
            .await
            .context("Failed to get container port")?;

        Ok(format!(
            "postgres://postgres:postgres@{}:{}/test_db",
            host, port
        ))
    }

    /// Waits for PostgreSQL to be ready and sets up the test database
    async fn wait_for_postgres_and_setup(
        container: &ContainerAsync<Postgres>,
    ) -> Result<()> {
        let host = container.get_host().await?;
        let port = container.get_host_port_ipv4(5432).await?;
        let default_connection_string = format!(
            "postgres://postgres:postgres@{}:{}/postgres",
            host, port
        );

        // Wait for PostgreSQL to be ready
        let pool =
            Self::wait_for_connection(&default_connection_string).await?;

        // Create test database
        Self::create_test_database(&pool).await?;

        Ok(())
    }

    /// Waits for PostgreSQL connection to be available
    async fn wait_for_connection(
        connection_string: &str,
    ) -> Result<sqlx::PgPool> {
        const MAX_ATTEMPTS: u32 = 20;
        const DELAY: Duration = Duration::from_millis(500);

        for attempt in 1..=MAX_ATTEMPTS {
            match PgPoolOptions::new()
                .max_connections(1)
                .acquire_timeout(Duration::from_secs(5))
                .connect(connection_string)
                .await
            {
                Ok(pool) => {
                    // Verify connection with a simple query
                    if sqlx::query("SELECT 1").fetch_one(&pool).await.is_ok()
                    {
                        return Ok(pool);
                    }
                }
                Err(_) if attempt < MAX_ATTEMPTS => {
                    sleep(DELAY).await;
                    continue;
                }
                Err(e) => {
                    return Err(e).context(format!(
                        "PostgreSQL not ready after {} attempts",
                        MAX_ATTEMPTS
                    ));
                }
            }
        }

        unreachable!("Loop should have returned or errored")
    }

    /// Creates the test database
    async fn create_test_database(pool: &sqlx::PgPool) -> Result<()> {
        // Drop database if it exists (cleanup from previous runs)
        sqlx::query("DROP DATABASE IF EXISTS test_db")
            .execute(pool)
            .await
            .context("Failed to drop existing test database")?;

        // Create the test database
        sqlx::query("CREATE DATABASE test_db")
            .execute(pool)
            .await
            .context("Failed to create test database")?;

        Ok(())
    }
}

/// Test configuration for database connection
#[derive(serde::Deserialize)]
pub struct TestDbConfig {
    pub connection_string: String,
}

impl sql_connection::DbConnectConfig for TestDbConfig {
    fn scheme(&self) -> &str { "postgresql" }

    fn username(&self) -> &str { "" }

    fn password(&self) -> &str { "" }

    fn host(&self) -> &str { "" }

    fn port(&self) -> u16 { 5432 }

    fn name(&self) -> &str { "" }

    fn uri(&self) -> &str { &self.connection_string }
}

impl sql_connection::DbOptionsConfig for TestDbConfig {
    fn max_conn(&self) -> Option<u32> { Some(10) }

    fn min_conn(&self) -> Option<u32> { Some(2) }

    fn sql_logger(&self) -> bool { false }
}
