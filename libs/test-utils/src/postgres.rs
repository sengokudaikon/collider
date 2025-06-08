use std::time::Duration;

use anyhow::{Context, Result};
use sea_orm::{Database, DatabaseConnection};
use sqlx::postgres::PgPoolOptions;
use testcontainers::{ContainerAsync, runners::AsyncRunner};
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
    pub async fn new() -> Result<Self> {
        let container = Postgres::default()
            .start()
            .await
            .context("Failed to start PostgreSQL container")?;

        let connection_string =
            Self::build_connection_string(&container).await?;

        Self::wait_for_postgres_and_setup(&container).await?;

        let connection = Database::connect(&connection_string)
            .await
            .context("Failed to create database connection")?;

        let instance = Self {
            container,
            connection,
            connection_string,
        };

        instance.setup_extensions().await?;

        instance.apply_migrations().await?;

        Ok(instance)
    }

    pub async fn execute_sql(&self, sql: &str) -> Result<()> {
        let sqlx_pool = self.connection.get_postgres_connection_pool();
        sqlx::query(sql)
            .execute(sqlx_pool)
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
        let sqlx_pool = self.connection.get_postgres_connection_pool();
        Ok(SqlMigrator::new(sqlx_pool.clone()))
    }

    async fn setup_extensions(&self) -> Result<()> {
        let sqlx_pool = self.connection.get_postgres_connection_pool();

        sqlx::query("CREATE EXTENSION IF NOT EXISTS pg_uuidv7")
            .execute(sqlx_pool)
            .await
            .context(
                "Failed to enable pg_uuidv7 extension. The test PostgreSQL \
                 container may not have this extension installed.",
            )?;

        Ok(())
    }

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

    async fn wait_for_postgres_and_setup(
        container: &ContainerAsync<Postgres>,
    ) -> Result<()> {
        let host = container.get_host().await?;
        let port = container.get_host_port_ipv4(5432).await?;
        let default_connection_string = format!(
            "postgres://postgres:postgres@{}:{}/postgres",
            host, port
        );

        let pool =
            Self::wait_for_connection(&default_connection_string).await?;

        Self::create_test_database(&pool).await?;

        Ok(())
    }

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

    async fn create_test_database(pool: &sqlx::PgPool) -> Result<()> {
        sqlx::query("DROP DATABASE IF EXISTS test_db")
            .execute(pool)
            .await
            .context("Failed to drop existing test database")?;

        sqlx::query("CREATE DATABASE test_db")
            .execute(pool)
            .await
            .context("Failed to create test database")?;

        Ok(())
    }
}

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
