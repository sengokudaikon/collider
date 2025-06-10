use std::{
    thread,
    time::{Duration as StdDuration, Instant},
};

use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use flume::{bounded, Receiver, Sender};
use rand::{rngs::SmallRng, Rng, SeedableRng};
use rayon::prelude::*;
use sqlx::{PgPool, Row};
use tokio::time::sleep;
use uuid::Uuid;

async fn create_pool_with_retry(database_url: &str) -> Result<PgPool> {
    const MAX_RETRIES: u32 = 5;
    const INITIAL_DELAY: u64 = 1000;

    for attempt in 1..=MAX_RETRIES {
        println!(
            "🔌 Attempting database connection (attempt {}/{})",
            attempt, MAX_RETRIES
        );

        let pool_options = sqlx::postgres::PgPoolOptions::new()
            .max_connections(20)
            .min_connections(5)
            .acquire_timeout(StdDuration::from_secs(30))
            .idle_timeout(StdDuration::from_secs(600))
            .max_lifetime(StdDuration::from_secs(1800))
            .test_before_acquire(true)
            .after_connect(|conn, _meta| {
                Box::pin(async move {
                    sqlx::query("SET statement_timeout = '600s'")
                        .execute(&mut *conn)
                        .await?;
                    sqlx::query("SET lock_timeout = '300s'")
                        .execute(&mut *conn)
                        .await?;
                    sqlx::query(
                        "SET idle_in_transaction_session_timeout = '300s'",
                    )
                    .execute(&mut *conn)
                    .await?;
                    Ok(())
                })
            });

        match pool_options.connect(database_url).await {
            Ok(pool) => {
                match sqlx::query("SELECT 1").fetch_one(&pool).await {
                    Ok(_) => {
                        println!(
                            "✅ Database connection pool established \
                             successfully"
                        );
                        println!(
                            "📊 Pool config: max_connections=20, \
                             acquire_timeout=30s"
                        );
                        return Ok(pool);
                    }
                    Err(e) => {
                        eprintln!("❌ Connection test failed: {}", e);
                        if attempt == MAX_RETRIES {
                            return Err(anyhow::anyhow!(
                                "Connection test failed after {} attempts: \
                                 {}",
                                MAX_RETRIES,
                                e
                            ));
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("❌ Connection attempt {} failed: {}", attempt, e);
                if attempt == MAX_RETRIES {
                    return Err(anyhow::anyhow!(
                        "Database connection failed after {} attempts: {}",
                        MAX_RETRIES,
                        e
                    ));
                }
            }
        }

        let delay = INITIAL_DELAY * 2_u64.pow(attempt - 1);
        println!("⏳ Waiting {}ms before retry...", delay);
        sleep(StdDuration::from_millis(delay)).await;
    }

    unreachable!()
}

#[tokio::main]
async fn main() -> Result<()> {
    let start_time = Instant::now();

    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgres@localhost:5432/postgres".to_string()
    });

    let pool = create_pool_with_retry(&database_url)
        .await
        .context("Failed to establish database connection after retries")?;

    sqlx::query("SET session_replication_role = replica")
        .execute(&pool)
        .await?;
    sqlx::query("ALTER TABLE events DISABLE TRIGGER ALL")
        .execute(&pool)
        .await?;
    sqlx::query("SET synchronous_commit = OFF")
        .execute(&pool)
        .await?;
    sqlx::query("TRUNCATE events, users, event_types CASCADE")
        .execute(&pool)
        .await?;

    let user_start = Instant::now();
    let user_base = Uuid::now_v7();
    let user_base_bytes = user_base.as_bytes();

    let user_uuids: Vec<Uuid> = (0..100)
        .map(|i| {
            let mut bytes = *user_base_bytes;
            let counter_bytes = (i as u64).to_be_bytes();
            bytes[8..16].copy_from_slice(&counter_bytes);
            Uuid::from_bytes(bytes)
        })
        .collect();

    let mut user_query = String::with_capacity(5000);
    user_query.push_str("INSERT INTO users (id, name, created_at) VALUES ");
    let now = Utc::now().format("%Y-%m-%d %H:%M:%S%.6f");

    for (i, user_id) in user_uuids.iter().enumerate() {
        if i > 0 {
            user_query.push(',');
        }
        user_query.push_str(&format!(
            "('{}','User{}','{}')",
            user_id,
            i + 1,
            now
        ));
    }

    sqlx::query(&user_query).execute(&pool).await?;
    println!("Users: {:.2}s", user_start.elapsed().as_secs_f64());

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

    let mut event_type_query = String::with_capacity(1000);
    event_type_query.push_str("INSERT INTO event_types (name) VALUES ");
    for (i, name) in event_types.iter().enumerate() {
        if i > 0 {
            event_type_query.push(',');
        }
        event_type_query.push_str(&format!("('{}')", name));
    }
    sqlx::query(&event_type_query).execute(&pool).await?;

    let event_type_rows =
        sqlx::query("SELECT id FROM event_types ORDER BY id")
            .fetch_all(&pool)
            .await?;
    let event_type_ids: Vec<i32> = event_type_rows
        .iter()
        .map(|row| row.get::<i32, _>("id"))
        .collect();
    
    let target_events = 10_000_000;

    let event_base = Uuid::now_v7();
    let event_base_bytes = event_base.as_bytes();

    let uuid_gen_start = Instant::now();
    let event_uuids: Vec<Uuid> = (0..target_events)
        .into_par_iter()
        .map(|i| {
            let mut bytes = *event_base_bytes;
            let counter_bytes = (i as u64).to_be_bytes();
            bytes[8..16].copy_from_slice(&counter_bytes);
            Uuid::from_bytes(bytes)
        })
        .collect();
    println!(
        "UUID generation: {:.2}s",
        uuid_gen_start.elapsed().as_secs_f64()
    );

    let batch_size = 10_000;
    let (sender, receiver) = bounded(5);

    let pool_clone = pool.clone();
    let consumer_task =
        tokio::spawn(async move { insert(&pool_clone, receiver).await });

    let user_uuids_clone = user_uuids.clone();
    let event_type_ids_clone = event_type_ids.clone();
    let producer_task = thread::spawn(move || {
        generate_events(
            target_events,
            &event_uuids,
            &user_uuids_clone,
            &event_type_ids_clone,
            sender,
            batch_size,
        );
    });

    producer_task.join().unwrap();
    let total_inserted = consumer_task.await??;

    sqlx::query("ALTER TABLE events ENABLE TRIGGER ALL")
        .execute(&pool)
        .await?;
    sqlx::query("SET session_replication_role = DEFAULT")
        .execute(&pool)
        .await?;
    sqlx::query("SET synchronous_commit = ON")
        .execute(&pool)
        .await?;

    let total_time = start_time.elapsed();
    let events_per_sec = total_inserted as f64 / total_time.as_secs_f64();

    println!(
        "results: {} events | {:.2}s | {:.0}/sec",
        total_inserted,
        total_time.as_secs_f64(),
        events_per_sec
    );

    Ok(())
}

fn generate_events(
    event_count: usize, event_uuids: &[Uuid], user_uuids: &[Uuid],
    event_type_ids: &[i32], sender: Sender<Vec<String>>, batch_size: usize,
) {
    let start_timestamp = (Utc::now() - Duration::days(30)).timestamp();
    let end_timestamp = Utc::now().timestamp();
    let time_range = end_timestamp - start_timestamp;

    let chunk_size = 1_000_000;
    let chunks: Vec<_> = (0..event_count).step_by(chunk_size).collect();

    chunks.into_par_iter().for_each(|chunk_start| {
        let chunk_end = (chunk_start + chunk_size).min(event_count);
        let mut rng = SmallRng::from_entropy();
        let mut batch = Vec::with_capacity(batch_size);

        for i in chunk_start..chunk_end {
            let user_id = user_uuids[i % user_uuids.len()];
            let event_type_id = event_type_ids[i % event_type_ids.len()];
            let random_offset = rng.gen_range(0..time_range);
            let timestamp = start_timestamp + random_offset;

            let metadata = match i % 3 {
                0 => format!(r#"{{"page":{}}}"#, i + 1),
                1 => format!(r#"{{"btn":{}}}"#, (i + 1) % 100),
                _ => format!(r#"{{"id":{}}}"#, i + 1),
            };

            let row = format!(
                "('{}','{}',{},to_timestamp({}),'{}'::jsonb)",
                event_uuids[i],
                user_id,
                event_type_id,
                timestamp,
                metadata.replace('\'', "''")
            );

            batch.push(row);

            if batch.len() == batch_size {
                if sender.send(batch).is_err() {
                    return;
                }
                batch = Vec::with_capacity(batch_size);
            }
        }

        if !batch.is_empty() {
            let _ = sender.send(batch);
        }
    });
}

async fn insert(
    pool: &PgPool, receiver: Receiver<Vec<String>>,
) -> Result<usize> {
    let mut total_inserted = 0;
    let mut batch_count = 0;

    while let Ok(batch) = receiver.recv() {
        batch_count += 1;

        if batch_count % 10 == 0 {
            match sqlx::query("SELECT 1").fetch_one(pool).await {
                Ok(_) => {
                    if batch_count % 50 == 0 {
                        println!(
                            "💚 Connection health check passed (batch {})",
                            batch_count
                        );
                    }
                }
                Err(e) => {
                    eprintln!(
                        "🚨 Connection health check failed at batch {}: {}",
                        batch_count, e
                    );
                    return Err(anyhow::anyhow!(
                        "Connection health check failed: {}",
                        e
                    ));
                }
            }
        }

        let mut query = String::with_capacity(batch.len() * 150 + 100);
        query.push_str(
            "INSERT INTO events (id, user_id, event_type_id, timestamp, \
             metadata) VALUES ",
        );

        for (i, row) in batch.iter().enumerate() {
            if i > 0 {
                query.push(',');
            }
            query.push_str(row);
        }

        const MAX_RETRIES: u32 = 5;
        let mut last_error = None;

        for retry in 0..MAX_RETRIES {
            match sqlx::query(&query).execute(pool).await {
                Ok(_) => {
                    total_inserted += batch.len();
                    if retry > 0 {
                        println!(
                            "🔄 Batch insert succeeded after {} retries \
                             (batch {})",
                            retry, batch_count
                        );
                    }
                    break;
                }
                Err(e) => {
                    last_error = Some(e);
                    if retry < MAX_RETRIES - 1 {
                        let error_msg =
                            last_error.as_ref().unwrap().to_string();
                        eprintln!(
                            "⚠️  Insert retry {}/{} (batch {}): {}",
                            retry + 1,
                            MAX_RETRIES,
                            batch_count,
                            error_msg
                        );

                        let delay = if error_msg.contains("pool timed out")
                            || error_msg.contains("EOF")
                        {
                            2000 * (retry + 1) as u64
                        }
                        else {
                            500 * (retry + 1) as u64
                        };

                        sleep(StdDuration::from_millis(delay)).await;

                        if error_msg.contains("pool timed out")
                            || error_msg.contains("EOF")
                        {
                            match sqlx::query("SELECT 1")
                                .fetch_one(pool)
                                .await
                            {
                                Ok(_) => {
                                    println!(
                                        "🔍 Connection recovered for retry"
                                    )
                                }
                                Err(health_err) => {
                                    eprintln!(
                                        "💥 Connection still unhealthy: {}",
                                        health_err
                                    )
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some(err) = last_error {
            return Err(anyhow::anyhow!(
                "Insert failed after {} retries (batch {}): {}",
                MAX_RETRIES,
                batch_count,
                err
            ));
        }

        if batch_count % 20 == 0 {
            println!(
                "📈 Progress: {} batches processed, {} events inserted",
                batch_count, total_inserted
            );
        }
    }

    Ok(total_inserted)
}
