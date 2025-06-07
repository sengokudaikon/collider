use std::sync::Arc;

use event_bus::{CacheEvent, EventBus, SystemEvent};
use tokio::time::{Duration, sleep};
use uuid::Uuid;

#[tokio::main]
async fn main() {
    let bus = EventBus::<SystemEvent>::new(10_000);

    let invalidated_keys =
        Arc::new(tokio::sync::Mutex::new(Vec::<String>::new()));

    let keys_clone = invalidated_keys.clone();
    bus.subscribe("cache_invalidate", move |event| {
        let keys = keys_clone.clone();
        async move {
            if let SystemEvent::Cache(cache_event) = event.payload {
                let mut k = keys.lock().await;
                match cache_event {
                    CacheEvent::Invalidate { pattern } => {
                        println!("🗑️  Invalidating cache key: {}", pattern);
                        k.push(pattern);
                    }
                    CacheEvent::InvalidatePattern { pattern } => {
                        println!(
                            "🧹 Invalidating cache pattern: {}",
                            pattern
                        );
                        k.push(format!("pattern:{}", pattern));
                    }
                    CacheEvent::BulkInvalidate { patterns } => {
                        println!(
                            "💥 Bulk invalidating {} cache keys",
                            patterns.len()
                        );
                        k.extend(patterns);
                    }
                    CacheEvent::Warm { key, .. } => {
                        println!("🔥 Warming cache for key: {}", key);
                    }
                }
            }
        }
    })
    .await
    .unwrap();

    let keys_clone2 = invalidated_keys.clone();
    bus.subscribe("user_updated", move |event| {
        let keys = keys_clone2.clone();
        async move {
            if let SystemEvent::UserUpdated { user_id, fields } =
                event.payload
            {
                println!("👤 User {} updated, fields: {:?}", user_id, fields);
                let mut k = keys.lock().await;
                k.push(format!("user:{}:*", user_id));
                if fields.contains(&"is_active".to_string()) {
                    k.push("users:active".to_string());
                }
            }
        }
    })
    .await
    .unwrap();

    bus.start_processing(100).await;

    println!("🚀 Event bus started, publishing cache events...\n");

    let user_id = Uuid::now_v7();
    bus.publish(
        "user_updated",
        user_id.to_string(),
        SystemEvent::UserUpdated {
            user_id,
            fields: vec!["name".to_string(), "is_active".to_string()],
        },
        None,
        None,
    )
    .await
    .unwrap();

    bus.publish(
        "cache_invalidate",
        "cache_manager",
        SystemEvent::Cache(CacheEvent::Invalidate {
            pattern: "analytics:dashboard".to_string(),
        }),
        None,
        None,
    )
    .await
    .unwrap();

    bus.publish(
        "cache_invalidate",
        "cache_manager",
        SystemEvent::Cache(CacheEvent::BulkInvalidate {
            patterns: vec![
                "events:user:123:*".to_string(),
                "analytics:user:123:*".to_string(),
                "user:profile:123".to_string(),
            ],
        }),
        None,
        None,
    )
    .await
    .unwrap();

    sleep(Duration::from_millis(100)).await;

    let metrics = bus.metrics();
    println!("\n📊 Event Bus Metrics:");
    println!("  Events Published: {}", metrics.events_published);
    println!("  Events Processed: {}", metrics.events_processed);
    println!("  Subscribers: {}", metrics.subscribers_count);
    println!("  Processing Errors: {}", metrics.processing_errors);

    let keys = invalidated_keys.lock().await;
    println!("\n🔑 Invalidated Cache Keys:");
    for key in keys.iter() {
        println!("  - {}", key);
    }

    println!("\n✅ Cache invalidation complete!");
}
