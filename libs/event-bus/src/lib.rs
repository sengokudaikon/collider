use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use flume::{Receiver, Sender};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{instrument, warn};
use uuid::Uuid;

/// Type alias for subscriber registry to avoid type complexity warning
type SubscriberRegistry<E> =
    Arc<RwLock<HashMap<String, Vec<Sender<DomainEvent<E>>>>>>;

/// High-performance event bus for cross-domain communication and cache
/// invalidation Uses flume channels for lock-free message passing with
/// minimal allocations
#[derive(Clone)]
pub struct EventBus<E> {
    /// High-throughput channel for domain events
    event_tx: Sender<DomainEvent<E>>,
    event_rx: Receiver<DomainEvent<E>>,

    /// Subscriber registry for event routing
    subscribers: SubscriberRegistry<E>,

    /// Performance metrics
    metrics: Arc<EventBusMetrics>,
}

/// Domain event wrapper with metadata for tracing and routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainEvent<E> {
    pub id: Uuid,
    pub event_type: String,
    pub aggregate_id: String,
    pub timestamp: u64,
    pub payload: E,
    pub correlation_id: Option<Uuid>,
    pub causation_id: Option<Uuid>,
}

/// Performance metrics for monitoring event throughput
#[derive(Default)]
pub struct EventBusMetrics {
    pub events_published: AtomicU64,
    pub events_processed: AtomicU64,
    pub subscribers_count: AtomicU64,
    pub processing_errors: AtomicU64,
}

impl<E> EventBus<E>
where
    E: Clone + Send + Sync + 'static,
{
    /// Create new high-performance event bus with bounded channel
    /// Channel size optimized for high-throughput scenarios
    pub fn new(channel_capacity: usize) -> Self {
        let (event_tx, event_rx) = flume::bounded(channel_capacity);

        Self {
            event_tx,
            event_rx,
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(EventBusMetrics::default()),
        }
    }

    /// Create unbounded event bus for maximum throughput
    /// Use with caution - can consume unlimited memory under load
    pub fn unbounded() -> Self {
        let (event_tx, event_rx) = flume::unbounded();

        Self {
            event_tx,
            event_rx,
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(EventBusMetrics::default()),
        }
    }

    /// Publish domain event with zero-copy semantics where possible
    #[instrument(skip_all)]
    pub async fn publish(
        &self, event_type: impl Into<String>,
        aggregate_id: impl Into<String>, payload: E,
        correlation_id: Option<Uuid>, causation_id: Option<Uuid>,
    ) -> Result<(), PublishError> {
        let event = DomainEvent {
            id: Uuid::now_v7(),
            event_type: event_type.into(),
            aggregate_id: aggregate_id.into(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64,
            payload,
            correlation_id,
            causation_id,
        };

        self.event_tx.try_send(event.clone()).map_err(|e| {
            match e {
                flume::TrySendError::Full(_) => PublishError::ChannelFull,
                flume::TrySendError::Disconnected(_) => {
                    PublishError::ChannelClosed
                }
            }
        })?;

        self.metrics
            .events_published
            .fetch_add(1, Ordering::Relaxed);

        self.route_to_subscribers(event).await;

        Ok(())
    }

    /// Subscribe to specific event types with typed handler
    pub async fn subscribe<H, F>(
        &self, event_type: &str, mut handler: H,
    ) -> Result<(), SubscribeError>
    where
        H: FnMut(DomainEvent<E>) -> F + Send + 'static,
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let (tx, rx) = flume::unbounded();

        {
            let mut subscribers = self.subscribers.write().await;
            subscribers
                .entry(event_type.to_string())
                .or_insert_with(Vec::new)
                .push(tx);
        }

        self.metrics
            .subscribers_count
            .fetch_add(1, Ordering::Relaxed);

        tokio::spawn(async move {
            while let Ok(event) = rx.recv_async().await {
                handler(event).await;
            }
        });

        Ok(())
    }

    /// Start high-performance event processing loop
    /// Processes events in batches for optimal throughput
    pub async fn start_processing(&self, batch_size: usize) {
        let rx = self.event_rx.clone();
        let metrics = self.metrics.clone();

        tokio::spawn(async move {
            let mut batch = Vec::with_capacity(batch_size);
            let mut last_flush = Instant::now();
            let flush_interval = Duration::from_millis(10);

            loop {
                while batch.len() < batch_size {
                    match rx.try_recv() {
                        Ok(event) => batch.push(event),
                        Err(flume::TryRecvError::Empty) => break,
                        Err(flume::TryRecvError::Disconnected) => return,
                    }
                }

                if !batch.is_empty()
                    && (batch.len() >= batch_size
                        || last_flush.elapsed() >= flush_interval)
                {
                    let processed = batch.len();
                    batch.clear();
                    metrics
                        .events_processed
                        .fetch_add(processed as u64, Ordering::Relaxed);
                    last_flush = Instant::now();
                }

                // Small yield to prevent busy-waiting
                if batch.is_empty() {
                    tokio::time::sleep(Duration::from_micros(100)).await;
                }
            }
        });
    }

    /// Route event to registered subscribers for the event type
    async fn route_to_subscribers(&self, event: DomainEvent<E>) {
        let subscribers = self.subscribers.read().await;

        if let Some(subs) = subscribers.get(&event.event_type) {
            for tx in subs {
                if let Err(e) = tx.try_send(event.clone()) {
                    match e {
                        flume::TrySendError::Full(_) => {
                            warn!(
                                "Subscriber channel full for event type: {}",
                                event.event_type
                            );
                        }
                        flume::TrySendError::Disconnected(_) => {
                            warn!(
                                "Subscriber disconnected for event type: {}",
                                event.event_type
                            );
                        }
                    }
                    self.metrics
                        .processing_errors
                        .fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    }

    /// Get current performance metrics
    pub fn metrics(&self) -> EventBusSnapshot {
        EventBusSnapshot {
            events_published: self
                .metrics
                .events_published
                .load(Ordering::Relaxed),
            events_processed: self
                .metrics
                .events_processed
                .load(Ordering::Relaxed),
            subscribers_count: self
                .metrics
                .subscribers_count
                .load(Ordering::Relaxed),
            processing_errors: self
                .metrics
                .processing_errors
                .load(Ordering::Relaxed),
        }
    }
}

/// Snapshot of event bus performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventBusSnapshot {
    pub events_published: u64,
    pub events_processed: u64,
    pub subscribers_count: u64,
    pub processing_errors: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum PublishError {
    #[error("Event channel is full")]
    ChannelFull,
    #[error("Event channel is closed")]
    ChannelClosed,
}

#[derive(Debug, thiserror::Error)]
pub enum SubscribeError {
    #[error("Failed to register subscriber")]
    RegistrationFailed,
}

/// Cache invalidation events for tiered cache system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheEvent {
    /// Invalidate specific cache key
    Invalidate { pattern: String },
    /// Invalidate all keys matching pattern
    InvalidatePattern { pattern: String },
    /// Bulk invalidation for batch operations
    BulkInvalidate { patterns: Vec<String> },
    /// Cache warming event
    Warm { key: String, data: Vec<u8> },
}

/// Domain-specific events for cross-service communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemEvent {
    /// User domain events
    UserCreated {
        user_id: Uuid,
    },
    UserUpdated {
        user_id: Uuid,
        fields: Vec<String>,
    },
    UserDeleted {
        user_id: Uuid,
    },

    /// Event domain events
    EventCreated {
        event_id: Uuid,
        user_id: Uuid,
        event_type: String,
    },
    EventsIngested {
        count: usize,
        user_ids: Vec<Uuid>,
    },

    /// Analytics events
    MetricsComputed {
        user_id: Uuid,
        metrics: Vec<String>,
    },
    DashboardUpdated {
        timestamp: u64,
    },

    /// Cache events
    Cache(CacheEvent),
}

#[cfg(test)]
mod tests {
    use tokio::time::{Duration, sleep};

    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct TestEvent {
        message: String,
    }

    #[tokio::test]
    async fn test_event_bus_publish_subscribe() {
        let bus = EventBus::<TestEvent>::new(100);
        let events_received = Arc::new(std::sync::Mutex::new(0));
        let events_received_clone = events_received.clone();

        bus.subscribe("test", move |_event: DomainEvent<TestEvent>| {
            let events_received = events_received_clone.clone();
            async move {
                let mut count = events_received.lock().unwrap();
                *count += 1;
            }
        })
        .await
        .unwrap();

        bus.start_processing(10).await;

        for i in 0..5 {
            bus.publish(
                "test",
                format!("aggregate_{}", i),
                TestEvent {
                    message: format!("Test message {}", i),
                },
                None,
                None,
            )
            .await
            .unwrap();
        }

        sleep(Duration::from_millis(100)).await;

        let final_count = *events_received.lock().unwrap();
        assert_eq!(final_count, 5);

        let metrics = bus.metrics();
        assert_eq!(metrics.events_published, 5);
        assert_eq!(metrics.subscribers_count, 1);
    }

    #[tokio::test]
    async fn test_event_bus_performance() {
        let bus = EventBus::<TestEvent>::unbounded();
        let start = Instant::now();

        for i in 0..10_000 {
            bus.publish(
                "perf_test",
                format!("aggregate_{}", i),
                TestEvent {
                    message: format!("Performance test {}", i),
                },
                None,
                None,
            )
            .await
            .unwrap();
        }

        let duration = start.elapsed();
        println!("Published 10k events in {:?}", duration);

        let metrics = bus.metrics();
        assert_eq!(metrics.events_published, 10_000);

        assert!(duration < Duration::from_millis(100));
    }

    #[tokio::test]
    async fn test_cache_invalidation_events() {
        let bus = EventBus::<SystemEvent>::new(1000);
        let invalidated_keys = Arc::new(std::sync::Mutex::new(Vec::new()));
        let invalidated_keys_clone = invalidated_keys.clone();

        bus.subscribe("cache_invalidate", move |event| {
            let keys = invalidated_keys_clone.clone();
            async move {
                if let SystemEvent::Cache(CacheEvent::Invalidate {
                    pattern,
                }) = event.payload
                {
                    let mut k = keys.lock().unwrap();
                    k.push(pattern);
                }
            }
        })
        .await
        .unwrap();

        bus.start_processing(50).await;

        bus.publish(
            "cache_invalidate",
            "cache_manager",
            SystemEvent::Cache(CacheEvent::Invalidate {
                pattern: "user:123:*".to_string(),
            }),
            None,
            None,
        )
        .await
        .unwrap();

        sleep(Duration::from_millis(50)).await;

        let keys = invalidated_keys.lock().unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0], "user:123:*");
    }
}
