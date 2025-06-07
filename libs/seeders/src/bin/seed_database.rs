use std::env;

use anyhow::Result;
use seeders::{EventSeeder, EventTypeSeeder, SeederRunner, UserSeeder};
use sql_connection::{
    SqlConnect, config::PostgresDbConfig, connect_postgres_db,
};
use tracing::{Level, info};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Starting database seeding process");

    // Get database configuration from environment or use defaults
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://user:password@localhost:5432/collider".to_string()
    });

    let config = PostgresDbConfig {
        uri: database_url,
        max_conn: Some(50),
        min_conn: Some(5),
        logger: false,
    };

    // Initialize database connection
    connect_postgres_db(&config).await?;
    info!("Connected to database successfully");

    // Create SqlConnect instance (uses the static connection)
    let db = SqlConnect::from_global();

    // Configure seeding parameters
    let min_users = env::var("MIN_USERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10_000);

    let max_users = env::var("MAX_USERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100_000);

    let min_event_types = env::var("MIN_EVENT_TYPES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(50);

    let max_event_types = env::var("MAX_EVENT_TYPES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(200);

    let target_events = env::var("TARGET_EVENTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10_000_000);

    let batch_size = env::var("BATCH_SIZE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10_000);

    info!("Seeding configuration:");
    info!("  Users: {} - {}", min_users, max_users);
    info!("  Event Types: {} - {}", min_event_types, max_event_types);
    info!("  Target Events: {}", target_events);
    info!("  Batch Size: {}", batch_size);

    // Create seeders
    let user_seeder = UserSeeder::new(db.clone(), min_users, max_users);
    let event_type_seeder =
        EventTypeSeeder::new(db.clone(), min_event_types, max_event_types);
    let event_seeder =
        EventSeeder::new(db.clone(), target_events, batch_size);

    // Create and run seeder runner
    let runner = SeederRunner::new(db)
        .add_seeder(Box::new(user_seeder))
        .add_seeder(Box::new(event_type_seeder))
        .add_seeder(Box::new(event_seeder));

    runner.run_all().await?;

    info!("Database seeding completed successfully!");
    Ok(())
}
