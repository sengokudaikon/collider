use std::{
    env,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Instant,
};

use anyhow::Result;
use chrono::{Duration, Utc};
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use flume::{bounded, Receiver, Sender};
use futures::future::try_join_all;
use itertools::Itertools;
use rand::{rngs::SmallRng, Rng, SeedableRng};
use rayon::prelude::*;
use tokio_postgres::{types::ToSql, NoTls};
use uuid::Uuid;

const BATCH_SIZE: usize = 10_000;
const NUM_WORKERS: usize = 20;

type PgPool = Pool;

async fn create_pool() -> Result<Pool> {
    let pg_cfg =
        env::var("DATABASE_URL")?.parse::<tokio_postgres::Config>()?;

    let mgr_config = ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
    };

    let mgr = Manager::from_config(pg_cfg, NoTls, mgr_config);

    let pool = Pool::builder(mgr)
        .max_size(300)
        .runtime(deadpool_postgres::Runtime::Tokio1)
        .build()?;

    Ok(pool)
}

#[tokio::main]
async fn main() -> Result<()> {
    let start_time = Instant::now();

    let pool = create_pool().await?;

    prepare_database(&pool).await?;
    let user_uuids = seed_users(&pool, 1000).await?;
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
    
    let base_event_types = vec![
        "page_view", "button_click", "form_submit", "login", "logout", "purchase", 
        "search", "download", "upload", "share", "like", "comment", "follow", 
        "message", "notification", "error", "signup", "profile_update", 
        "settings_change", "session_start", "video_play", "video_pause", 
        "video_complete", "scroll", "hover", "focus", "blur", "resize", 
        "print", "copy", "paste", "cut", "drag", "drop", "zoom_in", "zoom_out", 
        "refresh", "bookmark", "unbookmark", "favorite", "unfavorite", "rate", 
        "review", "cart_add", "cart_remove", "checkout_start", "checkout_complete", 
        "payment_success", "payment_failed", "subscription_start", "subscription_cancel", 
        "trial_start", "trial_end", "upgrade", "downgrade", "referral_click", 
        "referral_signup", "email_open", "email_click", "sms_click", "push_click", 
        "invite_sent", "invite_accepted", "invite_declined", "group_join", 
        "group_leave", "group_create", "group_delete", "post_create", "post_edit", 
        "post_delete", "post_share", "post_report", "user_block", "user_unblock", 
        "user_report", "user_follow", "user_unfollow", "chat_start", "chat_end", 
        "chat_message", "voice_call", "video_call", "screen_share", "file_share", 
        "location_share", "privacy_change", "theme_change", "language_change", 
        "timezone_change", "currency_change", "api_call", "webhook_received", 
        "export_data", "import_data", "backup_create", "backup_restore", 
        "maintenance_start", "maintenance_end", "system_alert", "system_warning", 
        "system_error", "performance_issue", "security_event", "audit_log"
    ];
    
    let mut rng = SmallRng::from_entropy();
    let mut event_types: Vec<String> = Vec::with_capacity(100);
    
    for base_type in &base_event_types {
        event_types.push(base_type.to_string());
    }
    
    let prefixes = ["user_", "admin_", "system_", "api_", "mobile_", "web_", "email_", "push_"];
    let actions = ["create", "update", "delete", "view", "click", "hover", "submit", "cancel", "start", "end", "complete", "fail", "retry", "timeout"];
    let objects = ["profile", "account", "document", "image", "video", "audio", "report", "dashboard", "settings", "notification", "alert", "task", "project", "team"];
    
    while event_types.len() < 100 {
        let prefix = prefixes[rng.gen_range(0..prefixes.len())];
        let action = actions[rng.gen_range(0..actions.len())];
        let object = objects[rng.gen_range(0..objects.len())];
        let event_type = format!("{}{}{}", prefix, action, object);
        
        if !event_types.contains(&event_type) {
            event_types.push(event_type);
        }
    }

    let mut sql = "INSERT INTO event_types (name) VALUES ".to_string();
    let mut params: Vec<Box<dyn ToSql + Sync + Send>> =
        Vec::with_capacity(event_types.len());
    for (i, name) in event_types.iter().enumerate() {
        sql.push_str(&format!("(${})", i + 1));
        if i < event_types.len() - 1 {
            sql.push(',');
        }
        params.push(Box::new(name.as_str()));
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
    pool: PgPool, rx: Receiver<EventBatch>, inserted: Arc<AtomicUsize>,
) -> Result<()> {
    while let Ok(batch) = rx.recv_async().await {
        if batch.is_empty() {
            continue;
        }

        let batch_size = batch.len();
        let client = pool.get().await?;

        let mut values = String::with_capacity(batch_size * 150);

        for (i, event) in batch.iter().enumerate() {
            if i > 0 {
                values.push(',');
            }
            let timestamp =
                chrono::DateTime::from_timestamp(event.timestamp, 0).unwrap();
            values.push_str(&format!(
                "('{}', '{}', {}, '{}', '{}')",
                event.id,
                event.user_id,
                event.event_type_id,
                timestamp.format("%Y-%m-%d %H:%M:%S%.6f%z"),
                event.metadata.replace('\'', "''")
            ));
        }

        let sql = format!(
            "INSERT INTO events (id, user_id, event_type_id, timestamp, \
             metadata) VALUES {}",
            values
        );

        client.execute(&sql, &[]).await?;
        inserted.fetch_add(batch_size, Ordering::Relaxed);
    }

    Ok(())
}
