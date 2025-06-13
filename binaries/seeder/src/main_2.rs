use std::{
    ops::Range,
    sync::Arc,
    time::Instant,
};

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use flume::{bounded, Receiver, Sender};
use futures::future::try_join_all;
use rand::{rngs::SmallRng, Rng, SeedableRng};
use rayon::prelude::*;
use sqlx::{types::Json, PgPool, Postgres, QueryBuilder, Row};
use uuid::Uuid;

// --- PARAMETERS ---
const BATCH_SIZE: usize = 10_000;
const MAX_CONNECTIONS: u32 = 300;
const NUM_WORKERS: usize = 20;

// --- INSTRUMENTATION ---
// A struct to hold timing information for one batch processed by one worker.
struct TimingEvent {
    worker_id: usize,
    start_time: Instant,
    gen_done_time: Instant,
    db_done_time: Instant,
}

// The data struct for the final batch
struct EventData {
    id: Uuid,
    user_id: Uuid,
    event_type_id: i32,
    timestamp: DateTime<Utc>,
    metadata: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let start_time = Instant::now();

    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgres@localhost:5434/postgres".to_string()
    });

    let pool = create_pool(&database_url).await?;

    prepare_database(&pool).await?;
    let user_uuids = Arc::new(seed_users(&pool, 100).await?);
    let event_type_ids = Arc::new(seed_event_types(&pool).await?);

    let total_events = 10_000_000;
    seed_events(&pool, total_events, user_uuids, event_type_ids).await?;

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

// --- UNCHANGED SETUP CODE ---
async fn create_pool(database_url: &str) -> Result<PgPool> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(MAX_CONNECTIONS)
        .min_connections(20)
        .acquire_timeout(std::time::Duration::from_secs(30))
        .connect(database_url)
        .await?;
    Ok(pool)
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
// --- END UNCHANGED SETUP CODE ---

fn produce_work_ranges(tx: Sender<Range<usize>>, total_events: usize) {
    for i in (0..total_events).step_by(BATCH_SIZE) {
        let end = (i + BATCH_SIZE).min(total_events);
        if tx.send(i..end).is_err() {
            break;
        }
    }
}

/// The instrumented worker task.
async fn worker_task(
    worker_id: usize,
    pool: PgPool,
    rx: Receiver<Range<usize>>,
    timing_tx: Sender<TimingEvent>, // Channel to send timing data
    event_uuids: Arc<Vec<Uuid>>,
    user_uuids: Arc<Vec<Uuid>>,
    event_type_ids: Arc<Vec<i32>>,
) -> Result<()> {
    let sql = "INSERT INTO events (id, user_id, event_type_id, timestamp, \
               metadata) ";
    let mut rng = SmallRng::from_entropy();

    while let Ok(range) = rx.recv_async().await {
        let start_time = Instant::now();

        let start_timestamp = (Utc::now() - Duration::days(30)).timestamp();
        let end_timestamp = Utc::now().timestamp();
        let time_range = end_timestamp - start_timestamp;

        let batch: Vec<EventData> = range
            .map(|i| {
                EventData {
                    id: event_uuids[i],
                    user_id: user_uuids[i % user_uuids.len()],
                    event_type_id: event_type_ids[i % event_type_ids.len()],
                    timestamp: DateTime::from_timestamp(
                        start_timestamp + rng.gen_range(0..time_range),
                        0,
                    )
                    .unwrap(),
                    metadata: format!(r#"{{"page":{}}}"#, i + 1),
                }
            })
            .collect();

        let gen_done_time = Instant::now();

        if batch.is_empty() {
            continue;
        }

        let mut query_builder: QueryBuilder<Postgres> =
            QueryBuilder::new(sql);
        query_builder.push_values(batch, |mut b, event| {
            b.push_bind(event.id)
                .push_bind(event.user_id)
                .push_bind(event.event_type_id)
                .push_bind(event.timestamp)
                .push_bind(Json(event.metadata));
        });

        query_builder.build().execute(&pool).await?;

        let db_done_time = Instant::now();

        // Send the timing data. This is a non-blocking send.
        let _ = timing_tx.send(TimingEvent {
            worker_id,
            start_time,
            gen_done_time,
            db_done_time,
        });
    }
    Ok(())
}

async fn seed_events(
    pool: &PgPool, total_events: usize, user_uuids: Arc<Vec<Uuid>>,
    event_type_ids: Arc<Vec<i32>>,
) -> Result<()> {
    let event_uuids: Arc<Vec<Uuid>> = Arc::new(
        (0..total_events)
            .into_par_iter()
            .map(|_| Uuid::now_v7())
            .collect(),
    );

    let (work_tx, work_rx) = bounded::<Range<usize>>(NUM_WORKERS * 2);
    let (timing_tx, timing_rx) =
        bounded::<TimingEvent>(total_events / BATCH_SIZE);

    // The logger task that collects all timing events.
    let logger_handle = tokio::spawn(async move {
        let mut timings = Vec::new();
        while let Ok(event) = timing_rx.recv_async().await {
            timings.push(event);
        }
        timings
    });

    let producer_handle = tokio::spawn(async move {
        produce_work_ranges(work_tx, total_events);
    });

    let mut worker_tasks = Vec::with_capacity(NUM_WORKERS);
    for worker_id in 0..NUM_WORKERS {
        worker_tasks.push(tokio::spawn(worker_task(
            worker_id,
            pool.clone(),
            work_rx.clone(),
            timing_tx.clone(),
            event_uuids.clone(),
            user_uuids.clone(),
            event_type_ids.clone(),
        )));
    }

    // Drop the original sender so the logger loop can finish once all workers
    // are done.
    drop(timing_tx);

    try_join_all(worker_tasks).await?;
    producer_handle.await?;

    // Wait for the logger to finish and get the timing data.
    let timings = logger_handle.await?;

    // --- ANALYSIS ---
    println!("\n--- PERFORMANCE ANALYSIS ---");
    if timings.is_empty() {
        println!("No timing data collected.");
        return Ok(());
    }

    let total_batches = timings.len();
    let mut total_gen_time_ms = 0.0;
    let mut total_db_time_ms = 0.0;
    let mut total_idle_time_ms = 0.0;

    // Sort timings by start time to calculate idle time between batches.
    let mut sorted_timings = timings;
    sorted_timings.sort_by_key(|t| t.start_time);

    let mut last_finish_time = sorted_timings[0].start_time;

    for event in &sorted_timings {
        let gen_time = event.gen_done_time.duration_since(event.start_time);
        let db_time = event.db_done_time.duration_since(event.gen_done_time);
        let idle_time = event.start_time.duration_since(last_finish_time);

        total_gen_time_ms += gen_time.as_secs_f64() * 1000.0;
        total_db_time_ms += db_time.as_secs_f64() * 1000.0;
        // Only count idle time if it's significant (more than a microsecond)
        if idle_time.as_secs_f64() > 0.0 {
            total_idle_time_ms += idle_time.as_secs_f64() * 1000.0;
        }

        last_finish_time = event.db_done_time;
    }

    println!("Total batches processed: {}", total_batches);
    println!(
        "Avg time per batch (ms): {:.2}",
        (total_gen_time_ms + total_db_time_ms) / total_batches as f64
    );
    println!(
        "  -> Avg Generation time (ms): {:.2}",
        total_gen_time_ms / total_batches as f64
    );
    println!(
        "  -> Avg Database wait time (ms): {:.2}",
        total_db_time_ms / total_batches as f64
    );
    println!("\nTotal time spent across all workers:");
    println!(
        "  -> In Data Generation: {:.2}s",
        total_gen_time_ms / 1000.0
    );
    println!("  -> In Database Await:  {:.2}s", total_db_time_ms / 1000.0);
    println!(
        "  -> In Idle/Wait:       {:.2}s (This is the sum of time between a \
         worker finishing and starting its next batch)",
        total_idle_time_ms / 1000.0
    );
    println!("--------------------------\n");

    Ok(())
}
