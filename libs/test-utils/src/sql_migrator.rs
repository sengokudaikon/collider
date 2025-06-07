use sqlx::{PgPool, Postgres, Transaction};

/// SQL-based migration system using .sql files
/// This is a simple, reliable migration system that uses plain SQL files
pub struct SqlMigrator {
    pool: PgPool,
}

impl SqlMigrator {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Run all migrations in order from domain-specific SQL files
    pub async fn run_all_migrations(&self) -> anyhow::Result<()> {
        // Create the migration tracking table
        self.create_migration_table().await?;

        // Define migrations in order using domain-specific SQL files
        let migrations = vec![
            ("001_create_users", include_str!("../../../domains/user/migrations/sql/001_create_users.sql")),
            ("002_create_event_types", include_str!("../../../domains/events/migrations/sql/002_create_event_types.sql")),
            ("003_create_events", include_str!("../../../domains/events/migrations/sql/003_create_events.sql")),
            ("004_create_analytics_views", include_str!("../../../domains/analytics/migrations/sql/004_create_analytics_views.sql")),
        ];

        for (migration_name, migration_sql) in migrations {
            if !self.is_migration_applied(migration_name).await? {
                println!("Running migration: {}", migration_name);

                // Run the migration in a transaction
                let mut tx = self.pool.begin().await?;

                // Execute the migration SQL by splitting on semicolons and running each statement
                self.execute_migration_sql(&mut tx, migration_sql).await
                    .map_err(|e| anyhow::anyhow!("Failed to run migration {}: {}", migration_name, e))?;

                // Record that this migration was applied
                sqlx::query(
                    "INSERT INTO _migrations (name, applied_at) VALUES ($1, NOW())"
                )
                    .bind(migration_name)
                    .execute(&mut *tx)
                    .await?;

                tx.commit().await?;
                println!("Migration {} completed successfully", migration_name);
            } else {
                println!("Migration {} already applied, skipping", migration_name);
            }
        }

        Ok(())
    }

    /// Create the migration tracking table
    async fn create_migration_table(&self) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS _migrations (
                id SERIAL PRIMARY KEY,
                name VARCHAR(255) NOT NULL UNIQUE,
                applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
        )
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Check if a migration has already been applied
    async fn is_migration_applied(&self, migration_name: &str) -> anyhow::Result<bool> {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM _migrations WHERE name = $1"
        )
            .bind(migration_name)
            .fetch_one(&self.pool)
            .await?;

        Ok(count.0 > 0)
    }

    /// Run a specific migration by name (for testing)
    pub async fn run_migration(&self, migration_name: &str, migration_sql: &str) -> anyhow::Result<()> {
        self.create_migration_table().await?;

        if !self.is_migration_applied(migration_name).await? {
            let mut tx = self.pool.begin().await?;

            self.execute_migration_sql(&mut tx, migration_sql).await
                .map_err(|e| anyhow::anyhow!("Failed to run migration {}: {}", migration_name, e))?;

            sqlx::query(
                "INSERT INTO _migrations (name, applied_at) VALUES ($1, NOW())"
            )
                .bind(migration_name)
                .execute(&mut *tx)
                .await?;

            tx.commit().await?;
        }

        Ok(())
    }

    /// List applied migrations
    pub async fn list_applied_migrations(&self) -> anyhow::Result<Vec<String>> {
        self.create_migration_table().await?;

        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM _migrations ORDER BY applied_at"
        )
            .fetch_all(&self.pool)
            .await?;

        Ok(rows.into_iter().map(|(name,)| name).collect())
    }

    /// Execute migration SQL by splitting statements and running them individually
    async fn execute_migration_sql(&self, tx: &mut Transaction<'_, Postgres>, migration_sql: &str) -> anyhow::Result<()> {
        // Split the SQL into individual statements
        let statements = self.split_sql_statements(migration_sql);

        println!("DEBUG: Found {} statements", statements.len());
        for (i, statement) in statements.iter().enumerate() {
            println!("DEBUG: Statement {}: {}", i, statement.trim());
        }

        for statement in statements {
            let trimmed = statement.trim();
            if !trimmed.is_empty() && !trimmed.starts_with("--") && !trimmed.starts_with("/*") {
                println!("Executing SQL: {}", trimmed);
                sqlx::query(trimmed)
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to execute SQL statement '{}': {}", trimmed, e))?;
            }
        }

        Ok(())
    }

    /// Split SQL into individual statements, handling multi-line statements
    fn split_sql_statements(&self, sql: &str) -> Vec<String> {
        let mut statements = Vec::new();
        let mut current_statement = String::new();
        let mut in_string = false;
        let mut in_function = false;
        let mut chars = sql.chars().peekable();

        while let Some(ch) = chars.next() {
            current_statement.push(ch);

            match ch {
                '\'' => {
                    // Toggle string state (but handle escaped quotes)
                    if chars.peek() == Some(&'\'') {
                        current_statement.push(chars.next().unwrap()); // Skip escaped quote
                    } else {
                        in_string = !in_string;
                    }
                }
                '$' if !in_string => {
                    // Check for function delimiter ($$)
                    if chars.peek() == Some(&'$') {
                        current_statement.push(chars.next().unwrap());
                        in_function = !in_function;
                    }
                }
                ';' if !in_string && !in_function => {
                    // End of statement
                    statements.push(current_statement.trim().to_string());
                    current_statement.clear();
                }
                _ => {}
            }
        }

        // Add the final statement if there's content
        if !current_statement.trim().is_empty() {
            statements.push(current_statement.trim().to_string());
        }

        statements
    }

    /// Run down migrations using .down.sql files
    pub async fn run_down_migrations(&self, migrations_to_rollback: &[&str]) -> anyhow::Result<()> {
        let down_migrations = vec![
            ("004_create_analytics_views", include_str!("../../../domains/analytics/migrations/sql/004_create_analytics_views.down.sql")),
            ("003_create_events", include_str!("../../../domains/events/migrations/sql/003_create_events.down.sql")),
            ("002_create_event_types", include_str!("../../../domains/events/migrations/sql/002_create_event_types.down.sql")),
            ("001_create_users", include_str!("../../../domains/user/migrations/sql/001_create_users.down.sql")),
        ];

        for (migration_name, down_sql) in down_migrations {
            if migrations_to_rollback.contains(&migration_name) {
                println!("Rolling back migration: {}", migration_name);

                let mut tx = self.pool.begin().await?;

                self.execute_migration_sql(&mut tx, down_sql).await
                    .map_err(|e| anyhow::anyhow!("Failed to rollback migration {}: {}", migration_name, e))?;

                // Remove from migration tracking
                sqlx::query("DELETE FROM _migrations WHERE name = $1")
                    .bind(migration_name)
                    .execute(&mut *tx)
                    .await?;

                tx.commit().await?;
                println!("Migration {} rolled back successfully", migration_name);
            }
        }

        Ok(())
    }

    /// Reset all migrations by running down migrations in reverse order
    pub async fn reset_all(&self) -> anyhow::Result<()> {
        println!("WARNING: Resetting all migrations - this will delete ALL data!");

        let applied_migrations = self.list_applied_migrations().await?;
        let migrations_to_rollback: Vec<&str> = applied_migrations.iter().map(|s| s.as_str()).collect();

        self.run_down_migrations(&migrations_to_rollback).await?;

        // Finally drop the migrations table itself
        sqlx::query("DROP TABLE IF EXISTS _migrations CASCADE")
            .execute(&self.pool)
            .await?;

        println!("All migrations reset successfully");
        Ok(())
    }
}