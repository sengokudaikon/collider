use analytics::RedisAnalyticsMetricsUpdater;
use flume::{Receiver, Sender};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, error, warn, instrument};
use user_events::UserAnalyticsEvent;

/// Background service that consumes UserAnalyticsEvents and feeds them to Redis analytics
pub struct UserAnalyticsIntegration {
    receiver: Receiver<UserAnalyticsEvent>,
    metrics_updater: RedisAnalyticsMetricsUpdater,
}

impl UserAnalyticsIntegration {
    /// Create a new analytics integration service
    pub fn new(receiver: Receiver<UserAnalyticsEvent>) -> Self {
        Self {
            receiver,
            metrics_updater: RedisAnalyticsMetricsUpdater::new(),
        }
    }

    /// Spawn the background analytics consumer task
    pub fn spawn_background_task(receiver: Receiver<UserAnalyticsEvent>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut integration = Self::new(receiver);
            integration.run().await;
        })
    }

    /// Main consumer loop - processes UserAnalyticsEvents from flume channel
    #[instrument(skip_all)]
    async fn run(&mut self) {
        info!("Starting User Analytics Integration background service");
        
        let mut batch = Vec::new();
        let batch_size = 100;
        let batch_timeout = Duration::from_millis(500);
        
        loop {
            // Try to collect a batch of events
            match self.collect_batch(&mut batch, batch_size, batch_timeout).await {
                Ok(batch_count) if batch_count > 0 => {
                    if let Err(e) = self.process_batch(&batch).await {
                        error!("Failed to process analytics batch: {}", e);
                    }
                    batch.clear();
                }
                Ok(_) => {
                    // Empty batch, continue
                }
                Err(e) => {
                    error!("Error collecting analytics batch: {}", e);
                    sleep(Duration::from_millis(1000)).await;
                }
            }
        }
    }

    /// Collect a batch of events from the channel with timeout
    async fn collect_batch(
        &self,
        batch: &mut Vec<UserAnalyticsEvent>,
        max_size: usize,
        timeout: Duration,
    ) -> Result<usize, flume::RecvError> {
        let start = tokio::time::Instant::now();
        
        // Try to get at least one event (blocking)
        match self.receiver.recv_async().await {
            Ok(event) => {
                batch.push(event);
            }
            Err(e) => return Err(e),
        }
        
        // Then try to get more events (non-blocking) until batch is full or timeout
        while batch.len() < max_size && start.elapsed() < timeout {
            match self.receiver.try_recv() {
                Ok(event) => {
                    batch.push(event);
                }
                Err(flume::TryRecvError::Empty) => {
                    // No more events available, wait a bit
                    sleep(Duration::from_millis(10)).await;
                }
                Err(flume::TryRecvError::Disconnected) => {
                    warn!("Analytics event channel disconnected");
                    break;
                }
            }
        }
        
        Ok(batch.len())
    }

    /// Process a batch of analytics events
    #[instrument(skip_all, fields(batch_size = batch.len()))]
    async fn process_batch(&mut self, batch: &[UserAnalyticsEvent]) -> Result<(), String> {
        info!("Processing analytics batch of {} events", batch.len());
        
        let mut success_count = 0;
        let mut error_count = 0;
        
        for event in batch {
            match self.metrics_updater.process_event(event.clone()).await {
                Ok(_) => {
                    success_count += 1;
                }
                Err(e) => {
                    error!("Failed to process analytics event: {}", e);
                    error_count += 1;
                }
            }
        }
        
        info!(
            "Analytics batch processed: {} success, {} errors", 
            success_count, error_count
        );
        
        if error_count > 0 {
            Err(format!("Failed to process {} out of {} events", error_count, batch.len()))
        } else {
            Ok(())
        }
    }
}

/// Factory for creating analytics integration
pub struct UserAnalyticsFactory;

impl UserAnalyticsFactory {
    /// Create analytics integration with flume channel
    /// Returns (sender_for_handlers, background_task_handle)
    pub fn create_integration() -> (Sender<UserAnalyticsEvent>, tokio::task::JoinHandle<()>) {
        let (sender, receiver) = flume::unbounded();
        let task_handle = UserAnalyticsIntegration::spawn_background_task(receiver);
        
        info!("User Analytics Integration initialized");
        (sender, task_handle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_analytics_integration_flow() {
        let (sender, _handle) = UserAnalyticsFactory::create_integration();
        
        // Test sending an event
        let event = UserAnalyticsEvent::UserCreated {
            user_id: Uuid::new_v4(),
            name: "Test User".to_string(),
            created_at: Utc::now(),
            registration_source: Some("test".to_string()),
        };
        
        // Should not block
        sender.send(event).expect("Failed to send analytics event");
        
        // Give background task a moment to process
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}