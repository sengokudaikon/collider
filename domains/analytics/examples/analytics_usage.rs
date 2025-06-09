use std::sync::Arc;

use analytics::{
    AggregationFilters, EventProcessingService, EventProcessor,
    EventsAnalytics, EventsAnalyticsService, TimeBucket,
};
use chrono::{Duration, Utc};
use events_dao::EventDao;
use events_models::EventActiveModel;
use redis_connection::{config::RedisDbConfig, connect_redis_db};
use sea_orm::ActiveValue::Set;
use sql_connection::{
    SqlConnect, config::PostgresDbConfig, connect_postgres_db,
};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_config = PostgresDbConfig {
        uri: std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgresql://postgres:password@localhost/collider".to_string()
        }),
        max_conn: Some(10),
        min_conn: Some(5),
        logger: false,
    };
    connect_postgres_db(&db_config).await?;
    let sql = SqlConnect::from_global();

    let redis_config = RedisDbConfig {
        host: std::env::var("REDIS_HOST")
            .unwrap_or_else(|_| "127.0.0.1".to_string()),
        port: std::env::var("REDIS_PORT")
            .unwrap_or_else(|_| "6379".to_string())
            .parse()
            .unwrap_or(6379),
        db: 0,
    };
    let _redis_pool = connect_redis_db(&redis_config).await?;

    let analytics = Arc::new(EventsAnalyticsService::new(sql.clone()));

    let event_dao = EventDao::new(sql);
    let processor = EventProcessor::new(event_dao, analytics.clone());

    let processing_service = EventProcessingService::new(processor);
    processing_service.start_background_services().await;

    let processor = &processing_service.processor;

    println!("Creating sample events...");
    let user_id = Uuid::new_v4();

    let events = vec![
        EventActiveModel {
            id: Set(Uuid::now_v7()),
            user_id: Set(user_id),
            event_type_id: Set(1),
            timestamp: Set(Utc::now()),
            metadata: Set(Some(serde_json::json!({"ip": "192.168.1.1"}))),
        },
        EventActiveModel {
            id: Set(Uuid::now_v7()),
            user_id: Set(user_id),
            event_type_id: Set(2),
            timestamp: Set(Utc::now()),
            metadata: Set(Some(serde_json::json!({"page": "/dashboard"}))),
        },
        EventActiveModel {
            id: Set(Uuid::now_v7()),
            user_id: Set(user_id),
            event_type_id: Set(3),
            timestamp: Set(Utc::now()),
            metadata: Set(Some(
                serde_json::json!({"button": "save", "form": "profile"}),
            )),
        },
    ];

    for active_model in events {
        processor.create_event(active_model).await?;
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    println!("\n=== Real-time Analytics ===");

    let now = Utc::now();

    let minute_metrics = analytics
        .get_real_time_metrics(TimeBucket::Minute, now, None)
        .await?;

    println!(
        "Current minute - Total events: {}, Unique users: {}",
        minute_metrics.total_events, minute_metrics.unique_users
    );

    let time_series = analytics
        .get_time_series(
            TimeBucket::Hour,
            now - Duration::hours(24),
            now,
            Some(AggregationFilters {
                event_types: Some(vec![
                    "type_1".to_string(),
                    "type_2".to_string(),
                ]),
                user_ids: None,
                metadata_filters: None,
            }),
        )
        .await?;

    println!("Hourly metrics for last 24h:");
    for (bucket, metrics) in time_series.iter().take(5) {
        println!(
            "  {}: {} events, {} users",
            bucket, metrics.total_events, metrics.unique_users
        );
    }

    println!("\n=== Complex Analytics (Materialized Views) ===");

    let summaries = analytics
        .get_hourly_summaries(
            now - Duration::hours(12),
            now,
            Some(vec![1, 2, 3]), // event type IDs
        )
        .await?;

    println!("Hourly summaries:");
    for summary in summaries.iter().take(3) {
        println!(
            "  {} at {}: {} events, {} users, {:.2} avg per user",
            summary.event_type,
            summary.hour,
            summary.total_events,
            summary.unique_users,
            summary.avg_events_per_user
        );
    }

    let activity = analytics
        .get_user_activity(Some(user_id), now - Duration::days(7), now)
        .await?;

    println!("User activity for {}:", user_id);
    for day in activity.iter().take(3) {
        println!(
            "  {}: {} events, types: {:?}",
            day.date.format("%Y-%m-%d"),
            day.total_events,
            day.event_types
        );
    }

    let popular = analytics
        .get_popular_events("last_7_days", Some(10))
        .await?;

    println!("Popular events:");
    for event in popular.iter().take(5) {
        println!(
            "  {}: {} total, {} users, growth: {:?}%",
            event.event_type,
            event.total_count,
            event.unique_users,
            event.growth_rate
        );
    }

    println!("\n=== Batch Processing ===");

    let batch_events: Vec<EventActiveModel> = (0..1000)
        .map(|i| {
            EventActiveModel {
                id: Set(Uuid::now_v7()),
                user_id: Set(Uuid::new_v4()),
                event_type_id: Set((i % 5) + 1),
                timestamp: Set(Utc::now()),
                metadata: Set(Some(serde_json::json!({
                    "batch_id": i / 100,
                    "sequence": i
                }))),
            }
        })
        .collect();

    let start = std::time::Instant::now();
    let results = processor.create_events_batch(batch_events).await?;
    let duration = start.elapsed();

    println!(
        "Processed {} events in {:?} ({:.2} events/sec)",
        results.len(),
        duration,
        results.len() as f64 / duration.as_secs_f64()
    );

    println!("\n=== Background Maintenance ===");

    analytics.refresh_materialized_views().await?;
    println!("Materialized views refreshed successfully");

    println!("\n=== Analytics System Ready for Production ===");
    println!("Features:");
    println!("- ✅ Real-time Redis aggregations with time bucketing");
    println!("- ✅ PostgreSQL materialized views for complex queries");
    println!("- ✅ Async processing pipeline for high throughput");
    println!("- ✅ Background maintenance tasks");
    println!("- ✅ Comprehensive analytics API");
    println!("- ✅ Optimized for millions of events");

    Ok(())
}
