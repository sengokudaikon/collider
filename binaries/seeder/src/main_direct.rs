/// THIS RUNS IN 80 on fresh db
use std::{sync::Arc, time::Instant};

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use flume::{bounded, Receiver, Sender};
use futures::future::try_join_all;
use rand::{rngs::SmallRng, Rng, SeedableRng};
use rayon::prelude::*;
use sqlx::{types::Json, PgPool, Row};
use uuid::Uuid;

const BATCH_SIZE: usize = 10_000;
const MAX_CONNECTIONS: u32 = 20;
const NUM_WORKERS: usize = 12;

struct EventData {
    id: Uuid,
    user_id: Uuid,
    event_type_id: i32,
    timestamp: DateTime<Utc>,
    metadata: String,
}

type EventBatch = Vec<EventData>;

#[tokio::main]
async fn main() -> Result<()> {
    let start_time = Instant::now();

    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgres@localhost:5434/postgres".to_string()
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

async fn create_pool(database_url: &str) -> Result<PgPool> {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(MAX_CONNECTIONS)
        .min_connections(10)
        .acquire_timeout(std::time::Duration::from_secs(30))
        .connect(database_url)
        .await
        .map_err(Into::into)
}

async fn prepare_database(pool: &PgPool) -> Result<()> {
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
    sqlx::query("ALTER TABLE events ENABLE TRIGGER ALL")
        .execute(pool)
        .await?;
    sqlx::query("SET session_replication_role = DEFAULT")
        .execute(pool)
        .await?;
    Ok(())
}

async fn seed_users(pool: &PgPool, count: usize) -> Result<Vec<Uuid>> {
    let user_uuids: Vec<Uuid> = (0..count).map(|_| Uuid::now_v7()).collect();
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
    let event_types = [
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
    query_builder.push_values(event_types, |mut b, name| {
        b.push_bind(name);
    });
    query_builder.build().execute(pool).await?;
    let rows = sqlx::query("SELECT id FROM event_types ORDER BY id")
        .fetch_all(pool)
        .await?;
    Ok(rows.iter().map(|row| row.get("id")).collect())
}

async fn seed_events_optimized(
    pool: &PgPool, total_events: usize, user_uuids: &[Uuid],
    event_type_ids: &[i32],
) -> Result<()> {
    let event_uuids = Arc::new(generate_event_uuids(total_events));
    let (tx, rx) = bounded::<EventBatch>(NUM_WORKERS * 2);

    let producer_handle = {
        let event_uuids = event_uuids.clone();
        let user_uuids = user_uuids.to_vec();
        let event_type_ids = event_type_ids.to_vec();
        tokio::spawn(async move {
            produce_event_batches(
                tx,
                total_events,
                &event_uuids,
                &user_uuids,
                &event_type_ids,
            );
        })
    };

    let mut worker_tasks = Vec::with_capacity(NUM_WORKERS);
    for _ in 0..NUM_WORKERS {
        let pool = pool.clone();
        let rx = rx.clone();
        let task = tokio::spawn(async move { worker_task(pool, rx).await });
        worker_tasks.push(task);
    }

    try_join_all(worker_tasks).await?;
    producer_handle.await?;
    Ok(())
}

fn generate_event_uuids(count: usize) -> Vec<Uuid> {
    (0..count).into_par_iter().map(|_| Uuid::now_v7()).collect()
}

fn produce_event_batches(
    tx: Sender<EventBatch>, total_events: usize, event_uuids: &[Uuid],
    user_uuids: &[Uuid], event_type_ids: &[i32],
) {
    let start_timestamp = (Utc::now() - Duration::days(30)).timestamp();
    let end_timestamp = Utc::now().timestamp();
    let time_range = end_timestamp - start_timestamp;
    let mut rng = SmallRng::from_entropy();

    for chunk_start in (0..total_events).step_by(BATCH_SIZE) {
        let chunk_end = (chunk_start + BATCH_SIZE).min(total_events);

        let mut batch = Vec::with_capacity(chunk_end - chunk_start);

        for i in chunk_start..chunk_end {
            let event = EventData {
                id: event_uuids[i],
                user_id: user_uuids[i % user_uuids.len()],
                event_type_id: event_type_ids[i % event_type_ids.len()],
                timestamp: chrono::DateTime::from_timestamp(
                    start_timestamp + rng.gen_range(0..time_range),
                    0,
                )
                .unwrap(),
                metadata: format!(r#"{{"page":{}}}"#, i + 1),
            };
            batch.push(event);
        }

        // Send the completed batch. If the channel is closed, stop producing.
        if tx.send(batch).is_err() {
            break;
        }
    }
}

async fn worker_task(pool: PgPool, rx: Receiver<EventBatch>) -> Result<()> {
    while let Ok(batch) = rx.recv_async().await {
        let batch_size = batch.len();
        if batch_size == 0 {
            continue;
        }

        let mut sql = String::from(
            "INSERT INTO events (id, user_id, event_type_id, timestamp, \
             metadata) VALUES ",
        );
        for i in 0..batch_size {
            let base = i * 5;
            sql.push_str(&format!(
                "(${}, ${}, ${}, ${}, ${})",
                base + 1,
                base + 2,
                base + 3,
                base + 4,
                base + 5
            ));
            if i < batch_size - 1 {
                sql.push_str(", ");
            }
        }

        let mut query = sqlx::query(&sql);
        for event in batch {
            query = query
                .bind(event.id)
                .bind(event.user_id)
                .bind(event.event_type_id)
                .bind(event.timestamp)
                .bind(Json(event.metadata));
        }

        query.execute(&pool).await?;
    }

    Ok(())
}
