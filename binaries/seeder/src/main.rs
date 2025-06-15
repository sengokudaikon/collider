use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use anyhow::Result;
use clap::Parser;
use deadpool_postgres::Pool;
use flume::{bounded, Receiver, Sender};
use futures::future::try_join_all;
use seeder::{
    create_event_types, create_events_for_batch, create_pool, create_users, prepare_database,
    restore_database, Event, EventType, User,
};
use tokio_postgres::types::ToSql;
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "seeder")]
#[command(about = "Database seeding tool for performance testing")]
struct Cli {
    #[arg(long, default_value = "1000", help = "Number of users to create")]
    users_count: usize,

    #[arg(long, default_value = "10000000", help = "Number of events to create")]
    events_count: usize,

    #[arg(long, default_value = "100", help = "Number of event types to create")]
    event_types_count: usize,

    #[arg(long, default_value = "6000", help = "Batch size for bulk inserts")]
    batch_size: usize,
}

#[derive(Debug, Clone, Copy)]
struct WorkerStats {
    batches_processed: u32,
    events_processed: u64,
    total_wait_time: Duration,
    total_build_time: Duration,
    total_db_time: Duration,
}

struct RunTimings {
    total_duration: Duration,
    generation_duration: Duration,
    load_duration: Duration,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let overall_start = Instant::now();

    let gen_start = Instant::now();
    let users = create_users(cli.users_count);
    let event_types = create_event_types(cli.event_types_count);
    let generation_duration = gen_start.elapsed();

    let load_start = Instant::now();
    let pool = create_pool().await?;
    prepare_database(&pool).await?;

    insert_users(&pool, &users).await?;
    insert_event_types(&pool, &event_types).await?;

    let event_type_map: HashMap<String, i32> = event_types
        .iter()
        .enumerate()
        .map(|(i, et)| (et.name.clone(), (i + 1) as i32))
        .collect();

    let mut worker_tasks = Vec::new();
    let worker_count = num_cpus::get();

    let buffer_size = worker_count * 3;
    let (tx, rx) = bounded(buffer_size);
    let producer_handle = tokio::spawn(produce_batches(
        tx,
        users.clone(),
        event_types.clone(),
        cli.events_count,
        cli.batch_size,
    ));
    for _ in 0..worker_count {
        worker_tasks.push(tokio::spawn(worker_task(
            pool.clone(),
            rx.clone(),
            event_type_map.clone(),
        )));
    }

    producer_handle.await??;
    let worker_results = try_join_all(worker_tasks).await?;
    let load_duration = load_start.elapsed();

    restore_database(&pool).await?;

    let timings = RunTimings {
        total_duration: overall_start.elapsed(),
        generation_duration,
        load_duration,
    };

    print_summary_report(
        worker_results.into_iter().collect::<Result<Vec<_>, _>>()?,
        timings,
        cli.events_count,
    );

    Ok(())
}

async fn produce_batches(
    tx: Sender<Vec<Event>>,
    users: Vec<User>,
    event_types: Vec<EventType>,
    events_count: usize,
    batch_size: usize,
) -> Result<()> {
    let total_batches = (events_count + batch_size - 1) / batch_size;

    for i in 0..total_batches {
        let current_batch_size = if i == total_batches - 1 {
            events_count % batch_size
        }
        else {
            batch_size
        };

        let batch = create_events_for_batch(
            current_batch_size,
            &users,
            &event_types,
            i * batch_size,
        );

        if tx.send_async(batch).await.is_err() {
            break;
        }
    }

    Ok(())
}

async fn worker_task(
    pool: Pool, rx: Receiver<Vec<Event>>,
    event_type_map: HashMap<String, i32>,
) -> Result<WorkerStats> {
    let mut stats = WorkerStats {
        batches_processed: 0,
        events_processed: 0,
        total_wait_time: Duration::ZERO,
        total_build_time: Duration::ZERO,
        total_db_time: Duration::ZERO,
    };

    let client = pool.get().await?;

    loop {
        let wait_start = Instant::now();
        let batch = match rx.recv_async().await {
            Ok(batch) => batch,
            Err(_) => break,
        };
        stats.total_wait_time += wait_start.elapsed();

        if batch.is_empty() {
            continue;
        }

        let build_start = Instant::now();
        let (sql, params) = build_insert_query(&batch, &event_type_map);
        let param_refs: Vec<&(dyn ToSql + Sync)> = params
            .iter()
            .map(|p| p.as_ref() as &(dyn ToSql + Sync))
            .collect();
        stats.total_build_time += build_start.elapsed();

        let db_start = Instant::now();
        client.execute(sql.as_str(), &param_refs).await?;
        stats.total_db_time += db_start.elapsed();

        stats.batches_processed += 1;
        stats.events_processed += batch.len() as u64;
    }

    Ok(stats)
}

fn build_insert_query(
    batch: &[Event], event_type_map: &HashMap<String, i32>,
) -> (String, Vec<Box<dyn ToSql + Sync + Send>>) {
    let mut sql = "INSERT INTO events (id, user_id, event_type_id, \
                   timestamp, metadata) VALUES "
        .to_string();
    let mut params: Vec<Box<dyn ToSql + Sync + Send>> =
        Vec::with_capacity(batch.len() * 5);

    for (i, event) in batch.iter().enumerate() {
        let p_base = i * 5;
        sql.push_str(&format!(
            "(${}, ${}, ${}, ${}, ${})",
            p_base + 1,
            p_base + 2,
            p_base + 3,
            p_base + 4,
            p_base + 5
        ));
        if i < batch.len() - 1 {
            sql.push(',');
        }

        params.push(Box::new(event.id));
        params.push(Box::new(event.user_id));
        params
            .push(Box::new(*event_type_map.get(&event.event_type).unwrap()));
        params.push(Box::new(event.timestamp));
        params.push(Box::new(event.metadata.clone()));
    }

    (sql, params)
}

fn print_summary_report(all_stats: Vec<WorkerStats>, timings: RunTimings, events_count: usize) {
    println!("\n--- 🏁 Overall Timing Breakdown ---");
    println!("{:<25} | {:>15}", "Stage", "Duration");
    println!("{:-<43}", "");
    println!(
        "{:<25} | {:>15.2?}",
        "1. User, event_types generation", timings.generation_duration
    );
    println!(
        "{:<25} | {:>15.2?}",
        "2. Events load (generate + insert)", timings.load_duration
    );
    println!("{:-<43}", "");
    println!(
        "{:<25} | {:>15.2?}",
        "TOTAL RUNTIME", timings.total_duration
    );

    let events_per_second = events_count as f64 / timings.total_duration.as_secs_f64();
    println!("\nOverall Throughput: {:.0} events/sec", events_per_second);

    let mut total_batches = 0;
    let mut total_wait = Duration::ZERO;
    let mut total_build = Duration::ZERO;
    let mut total_db = Duration::ZERO;

    for stats in &all_stats {
        total_batches += stats.batches_processed;
        total_wait += stats.total_wait_time;
        total_build += stats.total_build_time;
        total_db += stats.total_db_time;
    }
    let total_worker_time = total_wait + total_build + total_db;

    let worker_count = all_stats.len() as u32;

    println!("\n--- Worker Performance Analysis ---");
    println!(
        "Workers: {} | Avg batches per worker: {:.1} | Batches range: {}-{}",
        worker_count,
        total_batches as f64 / worker_count as f64,
        all_stats
            .iter()
            .map(|s| s.batches_processed)
            .min()
            .unwrap_or(0),
        all_stats
            .iter()
            .map(|s| s.batches_processed)
            .max()
            .unwrap_or(0)
    );

    println!("\nPer-Worker Average Times:");
    println!(
        "  -Waiting for Data: {:.2?} ({:.1}%)",
        total_wait / worker_count,
        (total_wait.as_secs_f64() / total_worker_time.as_secs_f64()) * 100.0
    );
    println!(
        "  -Building Queries:  {:.2?} ({:.1}%)",
        total_build / worker_count,
        (total_build.as_secs_f64() / total_worker_time.as_secs_f64()) * 100.0
    );
    println!(
        "  -Database Calls:   {:.2?} ({:.1}%)",
        total_db / worker_count,
        (total_db.as_secs_f64() / total_worker_time.as_secs_f64()) * 100.0
    );
    println!(
        "\nAvg DB time per batch: {:.2?} ({} ms)",
        total_db / total_batches,
        (total_db / total_batches).as_millis()
    );

    println!("\n--- Resource Usage ---");
    if let Some(peak) = get_memory_usage() {
        println!("Memory Usage (Peak): {}", format_memory(peak));
    }
    else {
        println!("Memory Usage: Not available on this platform.");
    }
    println!("CPU Cores Utilized: {}", num_cpus::get());
}

async fn insert_users(pool: &Pool, users: &[User]) -> Result<Vec<Uuid>> {
    let client = pool.get().await?;
    let count = users.len();

    let mut sql =
        "INSERT INTO users (id, name, created_at) VALUES ".to_string();
    let mut params: Vec<Box<dyn ToSql + Sync + Send>> =
        Vec::with_capacity(count * 3);
    let mut param_idx = 1;

    for user in users {
        sql.push_str(&format!(
            "(${}, ${}, ${}),",
            param_idx,
            param_idx + 1,
            param_idx + 2
        ));
        params.push(Box::new(user.id));
        params.push(Box::new(user.name.as_str()));
        params.push(Box::new(user.created_at));
        param_idx += 3;
    }
    sql.pop();

    let params_slice: Vec<&(dyn ToSql + Sync)> = params
        .iter()
        .map(|p| p.as_ref() as &(dyn ToSql + Sync))
        .collect();
    client.execute(sql.as_str(), &params_slice).await?;

    Ok(users.iter().map(|u| u.id).collect())
}

async fn insert_event_types(
    pool: &Pool, event_types: &[EventType],
) -> Result<Vec<i32>> {
    let client = pool.get().await?;

    let mut sql = "INSERT INTO event_types (name) VALUES ".to_string();
    let mut params: Vec<Box<dyn ToSql + Sync + Send>> =
        Vec::with_capacity(event_types.len());
    for (i, event_type) in event_types.iter().enumerate() {
        sql.push_str(&format!("(${})", i + 1));
        if i < event_types.len() - 1 {
            sql.push(',');
        }
        params.push(Box::new(event_type.name.as_str()));
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

#[cfg(target_os = "macos")]
fn get_memory_usage() -> Option<u64> {
    use std::mem;

    let mut rusage_info = unsafe { mem::zeroed::<libc::rusage>() };
    let rusage_result =
        unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut rusage_info) };

    if rusage_result == 0 {
        let peak_memory = rusage_info.ru_maxrss as u64;
        Some(peak_memory)
    }
    else {
        None
    }
}

#[cfg(target_os = "linux")]
fn get_memory_usage() -> Option<u64> {
    use std::fs;

    if let Ok(contents) = fs::read_to_string("/proc/self/status") {
        for line in contents.lines() {
            if line.starts_with("VmHWM:") {
                if let Some(value) = line.split_whitespace().nth(1) {
                    if let Ok(kb) = value.parse::<u64>() {
                        return Some(kb * 1024);
                    }
                }
            }
        }
    }
    None
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn get_memory_usage() -> Option<u64> { None }

fn format_memory(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    }
    else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    }
    else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    }
    else {
        format!("{} bytes", bytes)
    }
}
