use std::time::Instant;

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use flume::{bounded, Receiver};
use futures::future::try_join_all;
use itertools::Itertools;
use mimalloc::MiMalloc;
use rand::{rngs::SmallRng, Rng, SeedableRng};
use rayon::prelude::*;
use sqlx::{types::Json, PgPool, Postgres, QueryBuilder, Row};
use uuid::Uuid;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

const BATCH_SIZE: usize = 10_000;
const NUM_WORKERS: usize = 12;
const MAX_CONNECTIONS: u32 = 20;
type BatchData = Vec<(Uuid, Uuid, i32, DateTime<Utc>, String)>;

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
    sqlx::query(
        "TRUNCATE events, users, event_types RESTART IDENTITY CASCADE",
    )
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

fn generate_event_uuids(count: usize) -> Vec<Uuid> {
    (0..count).into_par_iter().map(|_| Uuid::now_v7()).collect()
}

async fn seed_events_optimized(
    pool: &PgPool, total_events: usize, user_uuids: &[Uuid],
    event_type_ids: &[i32],
) -> Result<()> {
    let event_uuids = std::sync::Arc::new(generate_event_uuids(total_events));
    let (tx, rx) = bounded::<BatchData>(NUM_WORKERS * 2);

    let user_uuids = user_uuids.to_vec();
    let event_type_ids = event_type_ids.to_vec();
    let producer_handle = tokio::task::spawn_blocking(move || {
        let start_timestamp = (Utc::now() - Duration::days(30)).timestamp();
        let end_timestamp = Utc::now().timestamp();
        let time_range = end_timestamp - start_timestamp;
        let mut rng = SmallRng::from_entropy();

        for chunk in &(0..total_events).chunks(BATCH_SIZE) {
            let batch: BatchData = chunk
                .map(|i| {
                    let event_id = event_uuids[i];
                    let user_id = user_uuids[i % user_uuids.len()];
                    let event_type_id =
                        event_type_ids[i % event_type_ids.len()];
                    let timestamp =
                        start_timestamp + rng.gen_range(0..time_range);
                    let dt = chrono::DateTime::from_timestamp(timestamp, 0)
                        .unwrap();
                    let metadata = format!(r#"{{"page":{}}}"#, i + 1);
                    (event_id, user_id, event_type_id, dt, metadata)
                })
                .collect();

            if tx.send(batch).is_err() {
                break;
            }
        }
    });

    let mut worker_tasks = Vec::with_capacity(NUM_WORKERS);
    for _ in 0..NUM_WORKERS {
        let pool = pool.clone();
        let rx = rx.clone();
        let task = tokio::spawn(async move { consumer_task(pool, rx).await });
        worker_tasks.push(task);
    }

    producer_handle.await?;
    try_join_all(worker_tasks).await?;

    Ok(())
}

async fn consumer_task(pool: PgPool, rx: Receiver<BatchData>) -> Result<()> {
    let sql = "INSERT INTO events (id, user_id, event_type_id, timestamp, \
               metadata) ";

    while let Ok(batch) = rx.recv_async().await {
        if batch.is_empty() {
            continue;
        }

        let mut query_builder: QueryBuilder<Postgres> =
            QueryBuilder::new(sql);

        query_builder.push_values(
            batch,
            |mut b, (id, user_id, event_type_id, dt, metadata)| {
                b.push_bind(id)
                    .push_bind(user_id)
                    .push_bind(event_type_id)
                    .push_bind(dt)
                    .push_bind(Json(metadata));
            },
        );

        query_builder.build().execute(&pool).await?;
    }

    Ok(())
}
