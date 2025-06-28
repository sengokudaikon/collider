use deadpool_postgres::Pool;
use tokio_postgres::Transaction;

pub struct SqlMigrator {
    pool: Pool,
}

impl SqlMigrator {
    pub fn new(pool: Pool) -> Self { Self { pool } }

    pub async fn run_all_migrations(&self) -> anyhow::Result<()> {
        self.create_migration_table().await?;

        let migrations = vec![
            (
                "001_create_users",
                include_str!(
                    "../../../domains/users/migrations/sql/001_create_users.\
                     sql"
                ),
            ),
            (
                "002_create_event_types",
                include_str!(
                    "../../../domains/events/migrations/sql/\
                     002_create_event_types.sql"
                ),
            ),
            (
                "003_create_events",
                include_str!(
                    "../../../domains/events/migrations/sql/\
                     003_create_events.sql"
                ),
            ),
            (
                "004_create_stats_materialized_view",
                include_str!(
                    "../../../domains/events/migrations/sql/\
                     004_create_stats_materialized_view.sql"
                ),
            ),
            (
                "005_add_indexes",
                include_str!(
                    "../../../domains/events/migrations/sql/\
                     005_add_indexes.sql"
                ),
            ),
        ];

        for (migration_name, migration_sql) in migrations {
            if !self.is_migration_applied(migration_name).await? {
                println!("Running migration: {}", migration_name);

                let mut client = self.pool.get().await?;
                let tx = client.transaction().await?;

                self.execute_migration_sql(&tx, migration_sql)
                    .await
                    .map_err(|e| {
                        anyhow::anyhow!(
                            "Failed to run migration {}: {}",
                            migration_name,
                            e
                        )
                    })?;

                tx.execute(
                    "INSERT INTO _migrations (name, applied_at) VALUES ($1, \
                     NOW())",
                    &[&migration_name],
                )
                .await?;

                tx.commit().await?;
                println!(
                    "Migration {} completed successfully",
                    migration_name
                );
            }
            else {
                println!(
                    "Migration {} already applied, skipping",
                    migration_name
                );
            }
        }

        Ok(())
    }

    async fn create_migration_table(&self) -> anyhow::Result<()> {
        let client = self.pool.get().await?;
        client
            .execute(
                r#"
            CREATE TABLE IF NOT EXISTS _migrations (
                id SERIAL PRIMARY KEY,
                name VARCHAR(255) NOT NULL UNIQUE,
                applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
                &[],
            )
            .await?;
        Ok(())
    }

    async fn is_migration_applied(
        &self, migration_name: &str,
    ) -> anyhow::Result<bool> {
        let client = self.pool.get().await?;
        let row = client
            .query_one(
                "SELECT COUNT(*) FROM _migrations WHERE name = $1",
                &[&migration_name],
            )
            .await?;

        let count: i64 = row.get(0);
        Ok(count > 0)
    }

    pub async fn run_migration(
        &self, migration_name: &str, migration_sql: &str,
    ) -> anyhow::Result<()> {
        self.create_migration_table().await?;

        if !self.is_migration_applied(migration_name).await? {
            let mut client = self.pool.get().await?;
            let tx = client.transaction().await?;

            self.execute_migration_sql(&tx, migration_sql)
                .await
                .map_err(|e| {
                    anyhow::anyhow!(
                        "Failed to run migration {}: {}",
                        migration_name,
                        e
                    )
                })?;

            tx.execute(
                "INSERT INTO _migrations (name, applied_at) VALUES ($1, \
                 NOW())",
                &[&migration_name],
            )
            .await?;

            tx.commit().await?;
        }

        Ok(())
    }

    pub async fn list_applied_migrations(
        &self,
    ) -> anyhow::Result<Vec<String>> {
        self.create_migration_table().await?;

        let client = self.pool.get().await?;
        let rows = client
            .query("SELECT name FROM _migrations ORDER BY applied_at", &[])
            .await?;

        Ok(rows.into_iter().map(|row| row.get(0)).collect())
    }

    async fn execute_migration_sql<'a>(
        &self, tx: &Transaction<'a>, migration_sql: &str,
    ) -> anyhow::Result<()> {
        let statements = self.split_sql_statements(migration_sql);

        println!("DEBUG: Found {} statements", statements.len());
        for (i, statement) in statements.iter().enumerate() {
            println!("DEBUG: Statement {}: {}", i, statement.trim());
        }

        for statement in statements {
            let trimmed = statement.trim();
            if !trimmed.is_empty()
                && !trimmed.starts_with("--")
                && !trimmed.starts_with("/*")
            {
                println!("Executing SQL: {}", trimmed);
                tx.execute(trimmed, &[]).await.map_err(|e| {
                    anyhow::anyhow!(
                        "Failed to execute SQL statement '{}': {}",
                        trimmed,
                        e
                    )
                })?;
            }
        }

        Ok(())
    }

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
                    if chars.peek() == Some(&'\'') {
                        current_statement.push(chars.next().unwrap());
                    }
                    else {
                        in_string = !in_string;
                    }
                }
                '$' if !in_string => {
                    if chars.peek() == Some(&'$') {
                        current_statement.push(chars.next().unwrap());
                        in_function = !in_function;
                    }
                }
                ';' if !in_string && !in_function => {
                    let trimmed = current_statement.trim();
                    if !trimmed.is_empty() {
                        statements.push(trimmed.to_string());
                    }
                    current_statement.clear();
                }
                _ => {}
            }
        }

        let trimmed = current_statement.trim();
        if !trimmed.is_empty() {
            statements.push(trimmed.to_string());
        }

        statements
    }

    pub async fn run_down_migrations(
        &self, migrations_to_rollback: &[&str],
    ) -> anyhow::Result<()> {
        let down_migrations = vec![
            (
                "005_add_indexes",
                include_str!(
                    "../../../domains/events/migrations/sql/\
                     005_add_indexes.down.sql"
                ),
            ),
            (
                "004_create_stats_materialized_view",
                include_str!(
                    "../../../domains/events/migrations/sql/\
                     004_create_stats_materialized_view.down.sql"
                ),
            ),
            (
                "003_create_events",
                include_str!(
                    "../../../domains/events/migrations/sql/\
                     003_create_events.down.sql"
                ),
            ),
            (
                "002_create_event_types",
                include_str!(
                    "../../../domains/events/migrations/sql/\
                     002_create_event_types.down.sql"
                ),
            ),
            (
                "001_create_users",
                include_str!(
                    "../../../domains/users/migrations/sql/001_create_users.\
                     down.sql"
                ),
            ),
        ];

        for (migration_name, down_sql) in down_migrations {
            if migrations_to_rollback.contains(&migration_name) {
                println!("Rolling back migration: {}", migration_name);

                let mut client = self.pool.get().await?;
                let tx = client.transaction().await?;

                self.execute_migration_sql(&tx, down_sql).await.map_err(
                    |e| {
                        anyhow::anyhow!(
                            "Failed to rollback migration {}: {}",
                            migration_name,
                            e
                        )
                    },
                )?;

                tx.execute(
                    "DELETE FROM _migrations WHERE name = $1",
                    &[&migration_name],
                )
                .await?;

                tx.commit().await?;
                println!(
                    "Migration {} rolled back successfully",
                    migration_name
                );
            }
        }

        Ok(())
    }

    pub async fn reset_all(&self) -> anyhow::Result<()> {
        println!(
            "WARNING: Resetting all migrations - this will delete ALL data!"
        );

        let applied_migrations = self.list_applied_migrations().await?;
        let migrations_to_rollback: Vec<&str> =
            applied_migrations.iter().map(|s| s.as_str()).collect();

        self.run_down_migrations(&migrations_to_rollback).await?;

        let client = self.pool.get().await?;
        client
            .execute("DROP TABLE IF EXISTS _migrations CASCADE", &[])
            .await?;

        println!("All migrations reset successfully");
        Ok(())
    }
}
