use std::time::Duration;

use anyhow::{Context, Result};
use sea_orm::{Database, DatabaseConnection};
use sqlx::postgres::PgPoolOptions;
use tokio::time::sleep;

use crate::sql_migrator::SqlMigrator;

pub struct TestPostgresContainer {
    pub connection: DatabaseConnection,
    pub connection_string: String,
    is_unique_db: bool,
    db_name: Option<String>,
}

impl TestPostgresContainer {
    pub async fn new() -> Result<Self> {
        let unique_db_name =
            format!("test_db_{}", uuid::Uuid::now_v7().simple());
        let base_connection =
            "postgres://postgres:postgres@localhost:5433/postgres";
        let unique_connection = format!(
            "postgres://postgres:postgres@localhost:5433/{}",
            unique_db_name
        );

        // Connect to default postgres database to create the unique test
        // database
        Self::wait_for_postgres_ready(base_connection).await?;
        let admin_pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(base_connection)
            .await
            .context("Failed to connect to postgres admin database")?;

        // Create the unique test database
        sqlx::query(&format!("CREATE DATABASE {}", unique_db_name))
            .execute(&admin_pool)
            .await
            .context("Failed to create unique test database")?;

        admin_pool.close().await;

        // Now connect to the unique database and set it up
        Self::new_with_connection_string(
            &unique_connection,
            true,
            Some(unique_db_name.clone()),
        )
        .await
    }

    pub async fn new_with_connection_string(
        connection_string: &str, is_unique_db: bool, db_name: Option<String>,
    ) -> Result<Self> {
        let connection_string = connection_string.to_string();

        Self::wait_for_postgres_ready(&connection_string).await?;

        let connection = Database::connect(&connection_string)
            .await
            .context("Failed to create database connection")?;

        let instance = Self {
            connection,
            connection_string,
            is_unique_db,
            db_name,
        };

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

    async fn wait_for_postgres_ready(connection_string: &str) -> Result<()> {
        Self::wait_for_connection(connection_string).await?;
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

    /// Manually cleanup the database if this is a unique database container
    pub async fn cleanup(&self) -> Result<()> {
        if self.is_unique_db {
            if let Some(db_name) = &self.db_name {
                cleanup_unique_database(db_name).await?;
            }
        }
        Ok(())
    }
}

impl Drop for TestPostgresContainer {
    fn drop(&mut self) {
        if self.is_unique_db {
            if let Some(db_name) = &self.db_name {
                // Spawn a blocking task to clean up the database
                let db_name = db_name.clone();
                tokio::spawn(async move {
                    if let Err(e) = cleanup_unique_database(&db_name).await {
                        eprintln!(
                            "Warning: Failed to cleanup test database {}: {}",
                            db_name, e
                        );
                    }
                });
            }
        }
    }
}

async fn cleanup_unique_database(db_name: &str) -> Result<()> {
    let base_connection =
        "postgres://postgres:postgres@localhost:5433/postgres";

    match PgPoolOptions::new()
        .max_connections(1)
        .connect(base_connection)
        .await
    {
        Ok(admin_pool) => {
            // Terminate all connections to the database first
            let terminate_query = format!(
                "SELECT pg_terminate_backend(pid) FROM pg_stat_activity \
                 WHERE datname = '{}' AND pid <> pg_backend_pid()",
                db_name
            );
            let _ = sqlx::query(&terminate_query).execute(&admin_pool).await;

            // Drop the database
            let drop_query = format!("DROP DATABASE IF EXISTS {}", db_name);
            if let Err(e) =
                sqlx::query(&drop_query).execute(&admin_pool).await
            {
                eprintln!("Failed to drop database {}: {}", db_name, e);
            }

            admin_pool.close().await;
        }
        Err(e) => {
            eprintln!(
                "Failed to connect to admin database for cleanup: {}",
                e
            );
        }
    }

    Ok(())
}

/// Cleanup all test databases matching the pattern test_db_*
pub async fn cleanup_all_test_databases() -> Result<()> {
    let base_connection =
        "postgres://postgres:postgres@localhost:5433/postgres";

    match PgPoolOptions::new()
        .max_connections(1)
        .connect(base_connection)
        .await
    {
        Ok(admin_pool) => {
            // Get all databases that match our test pattern
            let query = "SELECT datname FROM pg_database WHERE datname LIKE \
                         'test_db_%'";
            let rows: Vec<(String,)> = sqlx::query_as(query)
                .fetch_all(&admin_pool)
                .await
                .context("Failed to list test databases")?;

            for (db_name,) in rows {
                println!("Cleaning up test database: {}", db_name);

                // Terminate all connections to the database first
                let terminate_query = format!(
                    "SELECT pg_terminate_backend(pid) FROM pg_stat_activity \
                     WHERE datname = '{}' AND pid <> pg_backend_pid()",
                    db_name
                );
                let _ =
                    sqlx::query(&terminate_query).execute(&admin_pool).await;

                // Drop the database
                let drop_query =
                    format!("DROP DATABASE IF EXISTS {}", db_name);
                if let Err(e) =
                    sqlx::query(&drop_query).execute(&admin_pool).await
                {
                    eprintln!("Failed to drop database {}: {}", db_name, e);
                }
                else {
                    println!("Successfully cleaned up database: {}", db_name);
                }
            }

            admin_pool.close().await;
        }
        Err(e) => {
            eprintln!(
                "Failed to connect to admin database for cleanup: {}",
                e
            );
        }
    }

    Ok(())
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
