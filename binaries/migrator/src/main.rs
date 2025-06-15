use std::env;

use anyhow::Result;
use clap::{Parser, Subcommand};
use sql_connection::{
    config::PostgresDbConfig, connect_postgres_db, get_sql_pool,
};
use test_utils::SqlMigrator;
use tracing::{Level, info};

#[derive(Parser)]
#[command(name = "migrator")]
#[command(about = "Database migration tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(long, help = "Database URL (or use DATABASE_URL env var)")]
    database_url: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    Up,

    Down {
        #[arg(short, long, default_value = "1")]
        steps: usize,
    },

    Reset,

    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let cli = Cli::parse();

    let database_url = cli.database_url.unwrap_or_else(|| {
        env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgresql://postgres:postgres@localhost:5434/postgres"
                .to_string()
        })
    });

    let config = PostgresDbConfig {
        uri: database_url,
        max_conn: Some(10),
        min_conn: Some(2),
        logger: false,
    };

    connect_postgres_db(&config).await?;
    info!("Connected to database successfully");

    let pool = get_sql_pool();
    let migrator = SqlMigrator::new(pool.clone());

    match cli.command {
        Commands::Up => {
            info!("Running all pending migrations...");
            migrator.run_all_migrations().await?;
            info!("✓ All migrations completed successfully");
        }
        Commands::Down { steps } => {
            info!("Rolling back {} migration(s)...", steps);
            let applied = migrator.list_applied_migrations().await?;
            if applied.is_empty() {
                info!("No migrations to roll back");
                return Ok(());
            }

            let to_rollback = applied
                .iter()
                .rev()
                .take(steps)
                .map(|s| s.as_str())
                .collect::<Vec<_>>();

            if to_rollback.is_empty() {
                info!("No migrations to roll back");
            }
            else {
                info!("Rolling back: {:?}", to_rollback);
                migrator.run_down_migrations(&to_rollback).await?;
                info!("✓ Rollback completed successfully");
            }
        }
        Commands::Reset => {
            info!(
                "⚠️  WARNING: This will reset ALL migrations and DELETE ALL \
                 DATA!"
            );
            info!("Press Ctrl+C to cancel, or wait 5 seconds to continue...");
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

            migrator.reset_all().await?;
            info!("✓ All migrations reset successfully");
        }
        Commands::Status => {
            let applied = migrator.list_applied_migrations().await?;
            if applied.is_empty() {
                info!("No migrations have been applied");
            }
            else {
                info!("Applied migrations:");
                for migration in applied {
                    info!("  ✓ {}", migration);
                }
            }
        }
    }

    Ok(())
}
