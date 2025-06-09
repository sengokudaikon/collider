use anyhow::Result;
use clap::Parser;
use seeders::{
    Cli, Commands, EventSeeder, EventTypeSeeder, ProgressTracker, ProgressUI,
    SeederRunner, UserSeeder,
};
use sql_connection::{
    SqlConnect, config::PostgresDbConfig, connect_postgres_db,
};
use tracing::{Level, error, info};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Starting database seeding process");

    let config = PostgresDbConfig {
        uri: cli.get_database_url(),
        max_conn: Some(50),
        min_conn: Some(5),
        logger: false,
    };

    connect_postgres_db(&config).await?;
    info!("Connected to database successfully");

    let db = SqlConnect::from_global();

    match cli.command {
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
                min_users,
                max_users,
                min_event_types,
                max_event_types,
                target_events,
                event_batch_size,
                cli.quiet,
            )
            .await?;
        }
        Commands::Users {
            min_users,
            max_users,
        } => {
            run_user_seeder(db, min_users, max_users, cli.quiet).await?;
        }
        Commands::EventTypes {
            min_types,
            max_types,
        } => {
            run_event_type_seeder(db, min_types, max_types, cli.quiet)
                .await?;
        }
        Commands::Events {
            target_events,
            batch_size,
        } => {
            run_event_seeder(db, target_events, batch_size, cli.quiet)
                .await?;
        }
    }

    info!("Database seeding completed successfully!");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn run_all_seeders(
    db: SqlConnect, min_users: usize, max_users: usize,
    min_event_types: usize, max_event_types: usize, target_events: usize,
    event_batch_size: Option<usize>, quiet: bool,
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

    if quiet {
        let runner = SeederRunner::new(db)
            .add_seeder(Box::new(user_seeder))
            .add_seeder(Box::new(event_type_seeder))
            .add_seeder(Box::new(event_seeder));
        runner.run_all().await?;
    }
    else {
        // Try to initialize UI, fallback to quiet mode if it fails
        match ProgressUI::new() {
            Ok(mut progress_ui) => {
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

                let ui_result = progress_ui.run(progress_rx).await;
                let _ = runner_handle.await;

                if let Err(e) = ui_result {
                    error!("UI error: {}", e);
                }
            }
            Err(e) => {
                info!(
                    "Failed to initialize progress UI, falling back to \
                     quiet mode: {}",
                    e
                );
                let runner = SeederRunner::new(db)
                    .add_seeder(Box::new(user_seeder))
                    .add_seeder(Box::new(event_type_seeder))
                    .add_seeder(Box::new(event_seeder));
                runner.run_all().await?;
            }
        }
    }

    Ok(())
}

async fn run_user_seeder(
    db: SqlConnect, min_users: usize, max_users: usize, quiet: bool,
) -> Result<()> {
    info!("Seeding users: {} - {}", min_users, max_users);

    let user_seeder = UserSeeder::new(db.clone(), min_users, max_users);

    if quiet {
        let runner = SeederRunner::new(db).add_seeder(Box::new(user_seeder));
        runner.run_all().await?;
    }
    else {
        // Try to initialize UI, fallback to quiet mode if it fails
        match ProgressUI::new() {
            Ok(mut progress_ui) => {
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

                let ui_result = progress_ui.run(progress_rx).await;
                let _ = runner_handle.await;

                if let Err(e) = ui_result {
                    error!("UI error: {}", e);
                }
            }
            Err(e) => {
                info!(
                    "Failed to initialize progress UI, falling back to \
                     quiet mode: {}",
                    e
                );
                let runner =
                    SeederRunner::new(db).add_seeder(Box::new(user_seeder));
                runner.run_all().await?;
            }
        }
    }

    Ok(())
}

async fn run_event_type_seeder(
    db: SqlConnect, min_types: usize, max_types: usize, quiet: bool,
) -> Result<()> {
    info!("Seeding event types: {} - {}", min_types, max_types);

    let event_type_seeder =
        EventTypeSeeder::new(db.clone(), min_types, max_types);

    if quiet {
        let runner =
            SeederRunner::new(db).add_seeder(Box::new(event_type_seeder));
        runner.run_all().await?;
    }
    else {
        // Try to initialize UI, fallback to quiet mode if it fails
        match ProgressUI::new() {
            Ok(mut progress_ui) => {
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

                let ui_result = progress_ui.run(progress_rx).await;
                let _ = runner_handle.await;

                if let Err(e) = ui_result {
                    error!("UI error: {}", e);
                }
            }
            Err(e) => {
                info!(
                    "Failed to initialize progress UI, falling back to \
                     quiet mode: {}",
                    e
                );
                let runner = SeederRunner::new(db)
                    .add_seeder(Box::new(event_type_seeder));
                runner.run_all().await?;
            }
        }
    }

    Ok(())
}

async fn run_event_seeder(
    db: SqlConnect, target_events: usize, batch_size: Option<usize>,
    quiet: bool,
) -> Result<()> {
    info!("Seeding {} events", target_events);

    let event_seeder = if let Some(batch_size) = batch_size {
        EventSeeder::new(db.clone(), target_events, batch_size)
    }
    else {
        EventSeeder::new(db.clone(), target_events, 10_000)
    };

    if quiet {
        let runner = SeederRunner::new(db).add_seeder(Box::new(event_seeder));
        runner.run_all().await?;
    }
    else {
        // Try to initialize UI, fallback to quiet mode if it fails
        match ProgressUI::new() {
            Ok(mut progress_ui) => {
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

                let ui_result = progress_ui.run(progress_rx).await;
                let _ = runner_handle.await;

                if let Err(e) = ui_result {
                    error!("UI error: {}", e);
                }
            }
            Err(e) => {
                info!(
                    "Failed to initialize progress UI, falling back to \
                     quiet mode: {}",
                    e
                );
                let runner =
                    SeederRunner::new(db).add_seeder(Box::new(event_seeder));
                runner.run_all().await?;
            }
        }
    }

    Ok(())
}
