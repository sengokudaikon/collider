use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Instant,
};

use anyhow::Result;
use chrono::{Duration, Utc};
use flume::{bounded, Receiver, Sender};
use futures::future::try_join_all;
use rand::{rngs::SmallRng, Rng, SeedableRng};
use rayon::prelude::*;
use sqlx::{PgPool, Row};
use uuid::Uuid;

const BATCH_SIZE: usize = 10_000;
const MAX_CONNECTIONS: u32 = 1000;
const NUM_WORKERS: usize = 20;

async fn create_pool(database_url: &str) -> Result<PgPool> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(MAX_CONNECTIONS)
        .min_connections(20)
        .acquire_timeout(std::time::Duration::from_secs(30))
        .connect(database_url)
        .await?;

    Ok(pool)
}

#[tokio::main]
async fn main() -> Result<()> {
    let start_time = Instant::now();

    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgres@localhost:5432/postgres".to_string()
    });

    let pool = create_pool(&database_url).await?;

    prepare_database(&pool).await?;

    let user_uuids = seed_users(&pool, 100).await?;

    let event_type_ids = seed_event_types(&pool).await?;

    let total_events = 10_000_000;
    seed_events_optimized(&pool, total_events, &user_uuids, &event_type_ids)
        .await?;

    restore_database(&pool).await?;

    let elapsed = start_time.elapsed();
    println!(
        "✅ Seeding completed: {} events in {:.2}s ({:.0} events/sec)",
        total_events,
        elapsed.as_secs_f64(),
        total_events as f64 / elapsed.as_secs_f64()
    );

    Ok(())
}

async fn prepare_database(pool: &PgPool) -> Result<()> {
    // Session-level unsafe optimizations (server-level set in config)
    sqlx::query("SET session_replication_role = replica")
        .execute(pool)
        .await?;
    sqlx::query("ALTER TABLE events DISABLE TRIGGER ALL")
        .execute(pool)
        .await?;
    sqlx::query("TRUNCATE events, users, event_types CASCADE")
        .execute(pool)
        .await?;

    Ok(())
}

async fn restore_database(pool: &PgPool) -> Result<()> {
    // Restore session-level safety (server-level needs manual config change)
    sqlx::query("ALTER TABLE events ENABLE TRIGGER ALL")
        .execute(pool)
        .await?;
    sqlx::query("SET session_replication_role = DEFAULT")
        .execute(pool)
        .await?;

    Ok(())
}

async fn seed_users(pool: &PgPool, count: usize) -> Result<Vec<Uuid>> {
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

    let mut query_builder =
        sqlx::QueryBuilder::new("INSERT INTO users (id, name, created_at) ");

    query_builder.push_values(
        user_uuids.iter().enumerate(),
        |mut b, (i, uuid)| {
            b.push_bind(uuid)
                .push_bind(format!("User{}", i + 1))
                .push_bind(Utc::now());
        },
    );

    query_builder.build().execute(pool).await?;
    Ok(user_uuids)
}

async fn seed_event_types(pool: &PgPool) -> Result<Vec<i32>> {
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

    let mut query_builder =
        sqlx::QueryBuilder::new("INSERT INTO event_types (name) ");

    query_builder.push_values(event_types.iter(), |mut b, name| {
        b.push_bind(name);
    });

    query_builder.build().execute(pool).await?;

    let rows = sqlx::query("SELECT id FROM event_types ORDER BY id")
        .fetch_all(pool)
        .await?;

    let ids: Vec<i32> = rows.iter().map(|row| row.get("id")).collect();
    Ok(ids)
}

async fn seed_events_optimized(
    pool: &PgPool, total_events: usize, user_uuids: &[Uuid],
    event_type_ids: &[i32],
) -> Result<()> {
    let inserted = Arc::new(AtomicUsize::new(0));

    let event_uuids = generate_event_uuids(total_events);

    let (tx, rx) = bounded::<EventBatch>(NUM_WORKERS * 2);

    let producer_handle = tokio::spawn({
        let event_uuids = event_uuids.clone();
        let user_uuids = user_uuids.to_vec();
        let event_type_ids = event_type_ids.to_vec();

        async move {
            produce_event_batches(
                tx,
                total_events,
                &event_uuids,
                &user_uuids,
                &event_type_ids,
            );
        }
    });

    let mut worker_tasks = Vec::with_capacity(NUM_WORKERS);

    for _worker_id in 0..NUM_WORKERS {
        let pool = pool.clone();
        let rx = rx.clone();
        let inserted = inserted.clone();

        let task =
            tokio::spawn(
                async move { worker_task(pool, rx, inserted).await },
            );
        worker_tasks.push(task);
    }

    let (..) = tokio::try_join!(producer_handle, try_join_all(worker_tasks))?;

    Ok(())
}

fn generate_event_uuids(count: usize) -> Vec<Uuid> {
    let base = Uuid::now_v7();
    let base_bytes = base.as_bytes();

    (0..count)
        .into_par_iter()
        .map(|i| {
            let mut bytes = *base_bytes;
            let counter_bytes = (i as u64).to_be_bytes();
            bytes[8..16].copy_from_slice(&counter_bytes);
            Uuid::from_bytes(bytes)
        })
        .collect()
}

struct EventBatch {
    events: Vec<EventData>,
}

struct EventData {
    id: Uuid,
    user_id: Uuid,
    event_type_id: i32,
    timestamp: i64,
    metadata: String,
}

fn produce_event_batches(
    tx: Sender<EventBatch>, total_events: usize, event_uuids: &[Uuid],
    user_uuids: &[Uuid], event_type_ids: &[i32],
) {
    let start_timestamp = (Utc::now() - Duration::days(30)).timestamp();
    let end_timestamp = Utc::now().timestamp();
    let time_range = end_timestamp - start_timestamp;

    let mut rng = SmallRng::from_entropy();
    let mut batch = Vec::with_capacity(BATCH_SIZE);

    for i in 0..total_events {
        let event = EventData {
            id: event_uuids[i],
            user_id: user_uuids[i % user_uuids.len()],
            event_type_id: event_type_ids[i % event_type_ids.len()],
            timestamp: start_timestamp + rng.gen_range(0..time_range),
            metadata: match i % 3 {
                0 => format!(r#"{{"page":{}}}"#, i + 1),
                1 => format!(r#"{{"btn":{}}}"#, (i + 1) % 100),
                _ => format!(r#"{{"id":{}}}"#, i + 1),
            },
        };

        batch.push(event);

        if batch.len() == BATCH_SIZE || i == total_events - 1 {
            if tx.send(EventBatch { events: batch }).is_err() {
                break;
            }
            batch = Vec::with_capacity(BATCH_SIZE);
        }
    }
}

async fn worker_task(
    pool: PgPool, rx: Receiver<EventBatch>, inserted: Arc<AtomicUsize>,
) -> Result<()> {
    while let Ok(batch) = rx.recv() {
        let batch_size = batch.events.len();

        let placeholders: Vec<String> = (0..batch_size)
            .map(|i| {
                let base = i * 5;
                format!("(${}, ${}, ${}, ${}, ${})",
                        base + 1, base + 2, base + 3, base + 4, base + 5)
            })
            .collect();

        let query = format!(
            "INSERT INTO events (id, user_id, event_type_id, timestamp, metadata) VALUES {}",
            placeholders.join(",")
        );

        let mut query_builder = sqlx::query(&query);

        for event in batch.events {
            query_builder = query_builder
                .bind(event.id)
                .bind(event.user_id)
                .bind(event.event_type_id)
                .bind(chrono::DateTime::from_timestamp(event.timestamp, 0).unwrap())
                .bind(sqlx::types::Json(event.metadata));
        }

        query_builder.execute(&pool).await?;
        inserted.fetch_add(batch_size, Ordering::Relaxed);
    }

    Ok(())
}
