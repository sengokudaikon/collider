use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Instant,
};

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use deadpool_postgres::{
    Config, ManagerConfig, Pool, RecyclingMethod, Runtime,
};
use flume::{bounded, Receiver, Sender};
use futures::future::try_join_all;
use itertools::Itertools;
use rand::{rngs::SmallRng, Rng, SeedableRng};
use rayon::prelude::*;
use tokio_postgres::{types::ToSql, NoTls};
use uuid::Uuid;

const BATCH_SIZE: usize = 10_000;
const MAX_CONNECTIONS: usize = 300;
const NUM_WORKERS: usize = 20;

type PgPool = Pool;

async fn create_pool(database_url: &str) -> Result<PgPool> {
    let mut cfg = Config::new();
    cfg.url = Some(database_url.to_string());
    cfg.pool = Some(deadpool_postgres::PoolConfig::new(MAX_CONNECTIONS));
    cfg.manager = Some(ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
    });
    let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls)?;
    Ok(pool)
}

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
    seed_events(&pool, total_events, &user_uuids, &event_type_ids).await?;
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
    let client = pool.get().await?;
    client
        .batch_execute(
            "SET session_replication_role = replica;
         ALTER TABLE events DISABLE TRIGGER ALL;
         TRUNCATE events, users, event_types CASCADE;",
        )
        .await?;
    Ok(())
}

async fn restore_database(pool: &PgPool) -> Result<()> {
    let client = pool.get().await?;
    client
        .batch_execute(
            "ALTER TABLE events ENABLE TRIGGER ALL;
         SET session_replication_role = DEFAULT;
         SET synchronous_commit = ON;",
        )
        .await?;
    Ok(())
}

async fn seed_users(pool: &PgPool, count: usize) -> Result<Vec<Uuid>> {
    let client = pool.get().await?;
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

    let mut sql =
        "INSERT INTO users (id, name, created_at) VALUES ".to_string();
    let mut params: Vec<Box<dyn ToSql + Sync + Send>> =
        Vec::with_capacity(count * 3);
    let mut param_idx = 1;

    for (i, uuid) in user_uuids.iter().enumerate() {
        sql.push_str(&format!(
            "(${}, ${}, ${}),",
            param_idx,
            param_idx + 1,
            param_idx + 2
        ));
        params.push(Box::new(*uuid));
        params.push(Box::new(format!("User{}", i + 1)));
        params.push(Box::new(Utc::now()));
        param_idx += 3;
    }
    sql.pop();

    let params_slice: Vec<&(dyn ToSql + Sync)> = params
        .iter()
        .map(|p| p.as_ref() as &(dyn ToSql + Sync))
        .collect();
    client.execute(sql.as_str(), &params_slice).await?;
    Ok(user_uuids)
}

async fn seed_event_types(pool: &PgPool) -> Result<Vec<i32>> {
    let client = pool.get().await?;
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

    let mut sql = "INSERT INTO event_types (name) VALUES ".to_string();
    let mut params: Vec<Box<dyn ToSql + Sync + Send>> =
        Vec::with_capacity(event_types.len());
    for (i, name) in event_types.iter().enumerate() {
        sql.push_str(&format!("(${})", i + 1));
        if i < event_types.len() - 1 {
            sql.push(',');
        }
        params.push(Box::new(*name));
    }

    let params_slice: Vec<&(dyn ToSql + Sync)> = params
        .iter()
        .map(|p| p.as_ref() as &(dyn ToSql + Sync))
        .collect();
    client.execute(sql.as_str(), &params_slice).await?;

    let rows = client
        .query("SELECT id FROM event_types ORDER BY id", &[])
        .await?;
    Ok(rows.iter().map(|row| row.get("id")).collect())
}

async fn seed_events(
    pool: &PgPool, total_events: usize, user_uuids: &[Uuid],
    event_type_ids: &[i32],
) -> Result<()> {
    let start = Instant::now();
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
    for worker_id in 0..NUM_WORKERS {
        let pool = pool.clone();
        let rx = rx.clone();
        let inserted = inserted.clone();
        let task = tokio::spawn(async move {
            worker_task(worker_id, pool, rx, inserted).await
        });
        worker_tasks.push(task);
    }

    let (..) = tokio::try_join!(producer_handle, try_join_all(worker_tasks))?;

    let total_inserted = inserted.load(Ordering::Relaxed);
    println!(
        "✅ Events seeded: {} in {:.2}s ({:.0} events/sec)",
        total_inserted,
        start.elapsed().as_secs_f64(),
        total_inserted as f64 / start.elapsed().as_secs_f64()
    );
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

type EventBatch = Vec<EventData>;

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

    for chunk in &(0..total_events).chunks(BATCH_SIZE) {
        let batch: EventBatch = chunk
            .map(|i| {
                EventData {
                    id: event_uuids[i],
                    user_id: user_uuids[i % user_uuids.len()],
                    event_type_id: event_type_ids[i % event_type_ids.len()],
                    timestamp: start_timestamp + rng.gen_range(0..time_range),
                    metadata: format!(r#"{{"page":{}}}"#, i + 1),
                }
            })
            .collect();
        if tx.send(batch).is_err() {
            break;
        }
    }
}

async fn worker_task(
    worker_id: usize, pool: PgPool, rx: Receiver<EventBatch>,
    inserted: Arc<AtomicUsize>,
) -> Result<()> {
    let mut batch_count = 0;

    let client = pool.get().await?;

    while let Ok(batch) = rx.recv_async().await {
        if batch.is_empty() {
            continue;
        }
        let batch_size = batch.len();

        let mut sql = "INSERT INTO events (id, user_id, event_type_id, \
                       timestamp, metadata) VALUES "
            .to_string();

        let mut params_data: Vec<Box<dyn ToSql + Sync + Send>> =
            Vec::with_capacity(batch_size * 5);
        let mut param_idx = 1;

        for event in &batch {
            sql.push_str(&format!(
                "(${}, ${}, ${}, ${}, ${}::jsonb),",
                param_idx,
                param_idx + 1,
                param_idx + 2,
                param_idx + 3,
                param_idx + 4
            ));
            params_data.push(Box::new(event.id));
            params_data.push(Box::new(event.user_id));
            params_data.push(Box::new(event.event_type_id));
            params_data.push(Box::new(
                DateTime::from_timestamp(event.timestamp, 0).unwrap(),
            ));
            params_data.push(Box::new(event.metadata.clone()));
            param_idx += 5;
        }
        sql.pop();

        let params_slice: Vec<&(dyn ToSql + Sync)> = params_data
            .iter()
            .map(|p| p.as_ref() as &(dyn ToSql + Sync))
            .collect();

        client.execute(sql.as_str(), &params_slice).await?;

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
    Ok(())
}
