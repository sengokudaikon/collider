use std::env;

use anyhow::Result;
use clap::{Parser, Subcommand};
use database_traits::connection::GetDatabaseConnect;
use sql_connection::{
    SqlConnect, config::PostgresDbConfig, connect_postgres_db,
};
use test_utils::SqlMigrator;
use tracing::{Level, info};

mod tui;

#[derive(Parser)]
#[command(name = "migrator")]
#[command(about = "Database migration tool with interactive TUI")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

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

    Tui,
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

    let db = SqlConnect::from_global();
    let db_conn = db.get_connect();
    let sqlx_pool = db_conn.get_postgres_connection_pool();
    let migrator = SqlMigrator::new(sqlx_pool.clone());

    match cli.command {
        Some(Commands::Up) => {
            info!("Running all pending migrations...");
            migrator.run_all_migrations().await?;
            info!("✓ All migrations completed successfully");
        }
        Some(Commands::Down { steps }) => {
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
        Some(Commands::Reset) => {
            info!(
                "⚠️  WARNING: This will reset ALL migrations and DELETE ALL \
                 DATA!"
            );
            info!("Press Ctrl+C to cancel, or wait 5 seconds to continue...");
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

            migrator.reset_all().await?;
            info!("✓ All migrations reset successfully");
        }
        Some(Commands::Status) => {
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
        Some(Commands::Tui) | None => {
            info!("Starting interactive TUI...");
            tui::run_tui(migrator).await?;
        }
    }

    Ok(())
}
