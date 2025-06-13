use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Instant,
};

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use futures::future::try_join_all;
use rand::{Rng, SeedableRng, rngs::SmallRng};
use rayon::prelude::*;
use serde_json::{self, Value as JsonValue};
use tokio_postgres::NoTls;
use uuid::Uuid;

const BATCH_SIZE: usize = 10_000;
const NUM_WORKERS: usize = 14; // Match PHP's MAX_COROUTINES

async fn create_pool(database_url: &str) -> Result<Pool> {
    let config = database_url.parse::<tokio_postgres::Config>()?;

    let mgr_config = ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
    };

    let mgr = Manager::from_config(config, NoTls, mgr_config);

    let pool = Pool::builder(mgr)
        .max_size(300)
        .runtime(deadpool_postgres::Runtime::Tokio1)
        .build()?;

    Ok(pool)
}

#[tokio::main]
async fn main() -> Result<()> {
    let start_time = Instant::now();

    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgres@localhost:5434/postgres".to_string()
    });

    let pool = create_pool(&database_url).await?;

    // Prepare database for bulk loading
    prepare_database(&pool).await?;

    // Seed users
    let user_uuids = seed_users(&pool, 100).await?;

    // Seed event types
    let event_type_ids = seed_event_types(&pool).await?;

    // Seed events using optimized batch insert
    let total_events = 10_000_000;
    let actual_inserted =
        seed_events(&pool, total_events, &user_uuids, &event_type_ids)
            .await?;

    // Restore database settings
    restore_database(&pool).await?;

    let elapsed = start_time.elapsed();
    println!(
        "✅ Seeding completed: {} events in {:.2}s ({:.0} events/sec)",
        actual_inserted,
        elapsed.as_secs_f64(),
        actual_inserted as f64 / elapsed.as_secs_f64()
    );

    Ok(())
}

async fn prepare_database(pool: &Pool) -> Result<()> {
    let client = pool.get().await?;

    client
        .execute("SET session_replication_role = replica", &[])
        .await?;
    client
        .execute("ALTER TABLE events DISABLE TRIGGER ALL", &[])
        .await?;
    client
        .execute("TRUNCATE events, users, event_types CASCADE", &[])
        .await?;

    Ok(())
}

async fn restore_database(pool: &Pool) -> Result<()> {
    let client = pool.get().await?;

    client
        .execute("ALTER TABLE events ENABLE TRIGGER ALL", &[])
        .await?;
    client
        .execute("SET session_replication_role = DEFAULT", &[])
        .await?;
    client.execute("SET synchronous_commit = ON", &[]).await?;

    Ok(())
}

async fn seed_users(pool: &Pool, count: usize) -> Result<Vec<Uuid>> {
    let base_uuid = Uuid::now_v7();
    let base_bytes = base_uuid.as_bytes();

    let user_uuids: Vec<Uuid> = (0..count)
        .map(|i| {
            let mut bytes = *base_bytes;
            let counter_bytes = (i as u64).to_be_bytes();
            bytes[8..16].copy_from_slice(&counter_bytes);
            Uuid::from_bytes(bytes)
        })
        .collect();

    let client = pool.get().await?;

    // Simple approach: use unnest with arrays
    let ids: Vec<Uuid> = user_uuids.clone();
    let names: Vec<String> =
        (0..count).map(|i| format!("User{}", i + 1)).collect();
    let timestamps: Vec<DateTime<Utc>> = vec![Utc::now(); count];

    client
        .execute(
            "INSERT INTO users (id, name, created_at) SELECT * FROM \
             unnest($1::uuid[], $2::text[], $3::timestamptz[])",
            &[&ids, &names, &timestamps],
        )
        .await?;

    Ok(user_uuids)
}

async fn seed_event_types(pool: &Pool) -> Result<Vec<i32>> {
    let event_types = vec![
        "page_view",
        "button_click",
        "form_submit",
        "login",
        "logout",
        "purchase",
        "search",
        "download",
        "upload",
        "share",
        "like",
        "comment",
        "follow",
        "message",
        "notification",
        "error",
        "signup",
        "profile_update",
        "settings_change",
        "session_start",
    ];

    let client = pool.get().await?;

    client
        .execute(
            "INSERT INTO event_types (name) SELECT * FROM unnest($1::text[])",
            &[&event_types],
        )
        .await?;

    let rows = client
        .query("SELECT id FROM event_types ORDER BY id", &[])
        .await?;
    let ids: Vec<i32> = rows.iter().map(|row| row.get(0)).collect();
    Ok(ids)
}

async fn seed_events(
    pool: &Pool, total_events: usize, user_uuids: &[Uuid],
    event_type_ids: &[i32],
) -> Result<usize> {
    let start = Instant::now();

    // Pre-generate ALL events first to eliminate generation overhead
    println!("Pre-generating {} events...", total_events);
    let gen_start = Instant::now();
    let all_events: Vec<EventData> =
        generate_all_events(total_events, user_uuids, event_type_ids);
    println!(
        "Generation complete in {:.2}s",
        gen_start.elapsed().as_secs_f64()
    );

    let inserted = Arc::new(AtomicUsize::new(0));

    // Create multiple independent workers to avoid synchronization
    let mut workers = Vec::new();
    let events_per_worker = total_events / NUM_WORKERS;

    for worker_id in 0..NUM_WORKERS {
        let pool = pool.clone();
        let inserted = inserted.clone();

        let start_idx = worker_id * events_per_worker;
        let end_idx = if worker_id == NUM_WORKERS - 1 {
            total_events // Last worker gets remaining events
        }
        else {
            (worker_id + 1) * events_per_worker
        };

        let worker_events = all_events[start_idx..end_idx].to_vec();

        let task = tokio::spawn(async move {
            worker_task_independent(worker_id, pool, worker_events, inserted)
                .await
        });
        workers.push(task);
    }

    try_join_all(workers).await?;

    let total_inserted = inserted.load(Ordering::Relaxed);
    println!(
        "✅ Events seeded: {} in {:.2}s ({:.0} events/sec)",
        total_inserted,
        start.elapsed().as_secs_f64(),
        total_inserted as f64 / start.elapsed().as_secs_f64()
    );

    Ok(total_inserted)
}

fn generate_all_events(
    total_events: usize, user_uuids: &[Uuid], event_type_ids: &[i32],
) -> Vec<EventData> {
    let start_timestamp = (Utc::now() - Duration::days(30)).timestamp();
    let end_timestamp = Utc::now().timestamp();
    let time_range = end_timestamp - start_timestamp;

    // Generate UUIDs and events in parallel
    (0..total_events)
        .into_par_iter()
        .map(|i| {
            let mut rng = SmallRng::from_entropy();
            EventData {
                id: Uuid::now_v7(),
                user_id: user_uuids[i % user_uuids.len()],
                event_type_id: event_type_ids[i % event_type_ids.len()],
                timestamp: start_timestamp + rng.gen_range(0..time_range),
                metadata: format!(r#"{{"page":{}}}"#, i + 1),
            }
        })
        .collect()
}

#[derive(Clone)]
struct EventData {
    id: Uuid,
    user_id: Uuid,
    event_type_id: i32,
    timestamp: i64,
    metadata: String,
}

// New independent worker that doesn't wait for others
async fn worker_task_independent(
    worker_id: usize, pool: Pool, events: Vec<EventData>,
    inserted: Arc<AtomicUsize>,
) -> Result<()> {
    println!("Worker {} starting with {} events", worker_id, events.len());

    // Get a dedicated client for this worker
    let client = pool.get().await?;

    let mut batch_count = 0;

    // Process events in batches without waiting for other workers
    for batch in events.chunks(BATCH_SIZE) {
        let batch_size = batch.len();

        // Pre-allocate with exact capacity
        let mut ids = Vec::with_capacity(batch_size);
        let mut user_ids = Vec::with_capacity(batch_size);
        let mut event_type_ids = Vec::with_capacity(batch_size);
        let mut timestamps = Vec::with_capacity(batch_size);
        let mut metadata = Vec::with_capacity(batch_size);

        for event in batch {
            ids.push(event.id);
            user_ids.push(event.user_id);
            event_type_ids.push(event.event_type_id);
            timestamps
                .push(DateTime::from_timestamp(event.timestamp, 0).unwrap());
            // Convert string to JSON value for JSONB compatibility
            let json_metadata: JsonValue = serde_json::from_str(
                &event.metadata,
            )
            .unwrap_or_else(|_| serde_json::json!({"error": "invalid_json"}));
            metadata.push(json_metadata);
        }

        // Execute with unnest - much faster than individual inserts
        match client.execute(
            r#"
            INSERT INTO events (id, user_id, event_type_id, timestamp, metadata)
            SELECT * FROM unnest($1::uuid[], $2::uuid[], $3::int[], $4::timestamptz[], $5::jsonb[])
            "#,
            &[&ids, &user_ids, &event_type_ids, &timestamps, &metadata]
        ).await {
            Ok(rows_affected) => {
                println!("Worker {}: Batch {} inserted {} rows", worker_id, batch_count + 1, rows_affected);
            }
            Err(e) => {
                eprintln!("Worker {}: Database error: {}", worker_id, e);
                return Err(e.into());
            }
        }

        inserted.fetch_add(batch_size, Ordering::Relaxed);
        batch_count += 1;

        if batch_count % 10 == 0 {
            let total = inserted.load(Ordering::Relaxed);
            println!(
                "Worker {}: {} batches, {} total events",
                worker_id, batch_count, total
            );
        }
    }

    println!("Worker {} completed {} batches", worker_id, batch_count);
    Ok(())
}
