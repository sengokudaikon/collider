use std::env;

use anyhow::Result;
use database_traits::connection::GetDatabaseConnect;
use seeders::{EventSeeder, EventTypeSeeder, SeederRunner, UserSeeder};
use sql_connection::{
    SqlConnect, config::PostgresDbConfig, connect_postgres_db,
};
use test_utils::SqlMigrator;
use tracing::{Level, info};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Starting database migration and seeding process");

    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:password@localhost:5432/collider".to_string()
    });

    let config = PostgresDbConfig {
        uri: database_url,
        max_conn: Some(50),
        min_conn: Some(5),
        logger: false,
    };

    connect_postgres_db(&config).await?;
    info!("Connected to database successfully");

    let db = SqlConnect::from_global();
    let db_conn = db.get_connect();

    let sqlx_pool = db_conn.get_postgres_connection_pool();

    info!("Running database migrations...");
    let migrator = SqlMigrator::new(sqlx_pool.clone());
    migrator.run_all_migrations().await?;
    info!("✓ Database migrations completed successfully");

    let should_seed = env::var("SKIP_SEEDING")
        .map(|v| v.to_lowercase() != "true")
        .unwrap_or(true);

    if should_seed {
        info!("Starting database seeding...");

        let min_users = env::var("MIN_USERS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(100);

        let max_users = env::var("MAX_USERS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1000);

        let min_event_types = env::var("MIN_EVENT_TYPES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5);

        let max_event_types = env::var("MAX_EVENT_TYPES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10);

        let target_events = env::var("TARGET_EVENTS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10_000);

        let batch_size = env::var("BATCH_SIZE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1_000);

        info!("Seeding configuration:");
        info!("  Users: {} - {}", min_users, max_users);
        info!("  Event Types: {} - {}", min_event_types, max_event_types);
        info!("  Target Events: {}", target_events);
        info!("  Batch Size: {}", batch_size);

        let user_seeder = UserSeeder::new(db.clone(), min_users, max_users);
        let event_type_seeder = EventTypeSeeder::new(
            db.clone(),
            min_event_types,
            max_event_types,
        );
        let event_seeder =
            EventSeeder::new(db.clone(), target_events, batch_size);

        let runner = SeederRunner::new(db)
            .add_seeder(Box::new(user_seeder))
            .add_seeder(Box::new(event_type_seeder))
            .add_seeder(Box::new(event_seeder));

        runner.run_all().await?;
        info!("✓ Database seeding completed successfully!");
    }
    else {
        info!("Skipping database seeding (SKIP_SEEDING=true)");
    }

    info!("Database setup completed successfully!");
    Ok(())
}
