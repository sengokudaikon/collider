mod cli;
mod cli_progress;
mod prompts;
use std::time::Instant;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands, ProgressMode};
use cli_progress::CliProgress;
use prompts::SeederConfig;
use seeders::{
    EventSeeder, EventTypeSeeder, ProgressTracker, SeederRunner, UserSeeder,
};
use sql_connection::{
    config::PostgresDbConfig, connect_postgres_db, SqlConnect,
};
use tokio::signal;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn, Level};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Use RUST_LOG env var or default based on build profile
    let log_level = if cfg!(debug_assertions) {
        Level::INFO
    }
    else {
        Level::WARN // Less verbose in release builds
    };

    tracing_subscriber::fmt()
        .with_max_level(
            std::env::var("RUST_LOG")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(log_level),
        )
        .init();

    let start_time = Instant::now();
    info!("Starting database seeding process");

    let config = PostgresDbConfig {
        uri: cli.get_database_url(),
        max_conn: Some(50),
        min_conn: Some(5),
        logger: false,
    };

    let db_connection_start = Instant::now();
    connect_postgres_db(&config).await?;
    let db_connection_time = db_connection_start.elapsed();
    info!(
        "Connected to database successfully in {:.2}ms",
        db_connection_time.as_secs_f64() * 1000.0
    );

    let db = SqlConnect::from_global();

    let seeding_start = Instant::now();

    // Set up cancellation token for graceful shutdown
    let cancellation_token = CancellationToken::new();

    // Resolve missing arguments interactively or fail in quiet mode
    let resolved_command =
        SeederConfig::from_commands(cli.command, cli.mode.clone())?;

    // Set up graceful shutdown handling
    let seeding_future = async {
        match resolved_command {
            Commands::All {
                min_users,
                max_users,
                min_event_types,
                max_event_types,
                target_events,
                event_batch_size,
            } => {
                run_all_seeders(
                    db,
                    min_users.unwrap(),
                    max_users.unwrap(),
                    min_event_types.unwrap(),
                    max_event_types.unwrap(),
                    target_events.unwrap(),
                    event_batch_size,
                    cli.mode,
                    cancellation_token.clone(),
                )
                .await
            }
            Commands::Users {
                min_users,
                max_users,
            } => {
                run_user_seeder(
                    db,
                    min_users.unwrap(),
                    max_users.unwrap(),
                    cli.mode,
                    cancellation_token.clone(),
                )
                .await
            }
            Commands::EventTypes {
                min_types,
                max_types,
            } => {
                run_event_type_seeder(
                    db,
                    min_types.unwrap(),
                    max_types.unwrap(),
                    cli.mode,
                    cancellation_token.clone(),
                )
                .await
            }
            Commands::Events {
                target_events,
                batch_size,
            } => {
                run_event_seeder(
                    db,
                    target_events.unwrap(),
                    batch_size,
                    cli.mode,
                    cancellation_token.clone(),
                )
                .await
            }
        }
    };

    // Set up signal handling for graceful shutdown
    let shutdown_signal = async {
        let ctrl_c = signal::ctrl_c();

        #[cfg(unix)]
        let terminate = async {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("failed to install signal handler")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {
                warn!("Received Ctrl+C signal, initiating graceful shutdown...");
                cancellation_token.cancel();
            },
            _ = terminate => {
                warn!("Received terminate signal, initiating graceful shutdown...");
                cancellation_token.cancel();
            },
        }
    };

    // Add timeout for very large operations (2 hours)
    let timeout_duration = std::time::Duration::from_secs(2 * 60 * 60);

    // Run seeding with signal handling and timeout
    tokio::select! {
        result = seeding_future => {
            result?;
        },
        _ = shutdown_signal => {
            warn!("Seeding interrupted by signal. Data may be partially seeded.");
            info!("Tip: Use Ctrl+C to gracefully stop seeding");
            return Ok(());
        },
        _ = tokio::time::sleep(timeout_duration) => {
            error!("Seeding timed out after {} hours. Process may be hanging.", timeout_duration.as_secs() / 3600);
            warn!("Consider reducing batch size or target events for better performance.");
            return Err(anyhow::anyhow!("Seeding operation timed out"));
        },
    }

    let seeding_time = seeding_start.elapsed();

    let total_time = start_time.elapsed();
    info!("Database seeding completed successfully!");
    info!("Seeding time: {:.2}s", seeding_time.as_secs_f64());
    info!("Total time: {:.2}s", total_time.as_secs_f64());
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn run_all_seeders(
    db: SqlConnect, min_users: usize, max_users: usize,
    min_event_types: usize, max_event_types: usize, target_events: usize,
    event_batch_size: Option<usize>, mode: ProgressMode,
    cancellation_token: CancellationToken,
) -> Result<()> {
    info!("Seeding configuration:");
    info!("  Users: {} - {}", min_users, max_users);
    info!("  Event Types: {} - {}", min_event_types, max_event_types);
    info!("  Target Events: {}", target_events);

    let user_seeder = UserSeeder::new(db.clone(), min_users, max_users);
    let event_type_seeder =
        EventTypeSeeder::new(db.clone(), min_event_types, max_event_types);
    let event_seeder = if let Some(batch_size) = event_batch_size {
        EventSeeder::new(db.clone(), target_events, batch_size)
    }
    else {
        EventSeeder::new(db.clone(), target_events, 10_000)
    };

    match mode {
        ProgressMode::Quiet => {
            // No progress output, just run
            let runner = SeederRunner::new(db)
                .add_seeder(Box::new(user_seeder))
                .add_seeder(Box::new(event_type_seeder))
                .add_seeder(Box::new(event_seeder));
            runner.run_all().await?;
        }
        ProgressMode::Interactive => {
            // Use CLI progress bars for interactive mode
            let mut cli_progress = CliProgress::new();
            let (progress_tracker, progress_rx) = ProgressTracker::new();

            let runner = SeederRunner::new(db)
                .with_progress(progress_tracker.clone())
                .add_seeder(Box::new(user_seeder))
                .add_seeder(Box::new(event_type_seeder))
                .add_seeder(Box::new(event_seeder));

            let runner_handle = tokio::spawn(async move {
                if let Err(e) = runner.run_all().await {
                    error!("Seeding failed: {}", e);
                    progress_tracker
                        .error("Runner".to_string(), e.to_string());
                }
                progress_tracker.finish();
            });

            let progress_result =
                cli_progress.run(progress_rx, cancellation_token).await;
            let _ = runner_handle.await;

            if let Err(e) = progress_result {
                error!("Progress display error: {}", e);
            }
        }
    }

    Ok(())
}

async fn run_user_seeder(
    db: SqlConnect, min_users: usize, max_users: usize, mode: ProgressMode,
    cancellation_token: CancellationToken,
) -> Result<()> {
    info!("Seeding users: {} - {}", min_users, max_users);

    let user_seeder = UserSeeder::new(db.clone(), min_users, max_users);

    match mode {
        ProgressMode::Quiet => {
            let runner =
                SeederRunner::new(db).add_seeder(Box::new(user_seeder));
            runner.run_all().await?;
        }
        ProgressMode::Interactive => {
            let mut cli_progress = CliProgress::new();
            let (progress_tracker, progress_rx) = ProgressTracker::new();

            let runner = SeederRunner::new(db)
                .with_progress(progress_tracker.clone())
                .add_seeder(Box::new(user_seeder));

            let runner_handle = tokio::spawn(async move {
                if let Err(e) = runner.run_all().await {
                    error!("Seeding failed: {}", e);
                    progress_tracker
                        .error("Runner".to_string(), e.to_string());
                }
                progress_tracker.finish();
            });

            let progress_result =
                cli_progress.run(progress_rx, cancellation_token).await;
            let _ = runner_handle.await;

            if let Err(e) = progress_result {
                error!("Progress display error: {}", e);
            }
        }
    }

    Ok(())
}

async fn run_event_type_seeder(
    db: SqlConnect, min_types: usize, max_types: usize, mode: ProgressMode,
    cancellation_token: CancellationToken,
) -> Result<()> {
    info!("Seeding event types: {} - {}", min_types, max_types);

    let event_type_seeder =
        EventTypeSeeder::new(db.clone(), min_types, max_types);

    match mode {
        ProgressMode::Quiet => {
            let runner =
                SeederRunner::new(db).add_seeder(Box::new(event_type_seeder));
            runner.run_all().await?;
        }
        ProgressMode::Interactive => {
            let mut cli_progress = CliProgress::new();
            let (progress_tracker, progress_rx) = ProgressTracker::new();

            let runner = SeederRunner::new(db)
                .with_progress(progress_tracker.clone())
                .add_seeder(Box::new(event_type_seeder));

            let runner_handle = tokio::spawn(async move {
                if let Err(e) = runner.run_all().await {
                    error!("Seeding failed: {}", e);
                    progress_tracker
                        .error("Runner".to_string(), e.to_string());
                }
                progress_tracker.finish();
            });

            let progress_result =
                cli_progress.run(progress_rx, cancellation_token).await;
            let _ = runner_handle.await;

            if let Err(e) = progress_result {
                error!("Progress display error: {}", e);
            }
        }
    }

    Ok(())
}

async fn run_event_seeder(
    db: SqlConnect, target_events: usize, batch_size: Option<usize>,
    mode: ProgressMode, cancellation_token: CancellationToken,
) -> Result<()> {
    info!("Seeding {} events", target_events);

    let event_seeder = if let Some(batch_size) = batch_size {
        EventSeeder::new(db.clone(), target_events, batch_size)
    }
    else {
        EventSeeder::new(db.clone(), target_events, 10_000)
    };

    match mode {
        ProgressMode::Quiet => {
            let runner =
                SeederRunner::new(db).add_seeder(Box::new(event_seeder));
            runner.run_all().await?;
        }
        ProgressMode::Interactive => {
            let mut cli_progress = CliProgress::new();
            let (progress_tracker, progress_rx) = ProgressTracker::new();

            let runner = SeederRunner::new(db)
                .with_progress(progress_tracker.clone())
                .add_seeder(Box::new(event_seeder));

            let runner_handle = tokio::spawn(async move {
                if let Err(e) = runner.run_all().await {
                    error!("Seeding failed: {}", e);
                    progress_tracker
                        .error("Runner".to_string(), e.to_string());
                }
                progress_tracker.finish();
            });

            let progress_result =
                cli_progress.run(progress_rx, cancellation_token).await;
            let _ = runner_handle.await;

            if let Err(e) = progress_result {
                error!("Progress display error: {}", e);
            }
        }
    }

    Ok(())
}
