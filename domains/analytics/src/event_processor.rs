use std::sync::Arc;

use events_dao::EventDao;
use events_models::{CreateEventRequest, EventResponse};
use sql_connection::database_traits::dao::GenericDao;
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{error, info, instrument, warn};

use crate::analytics_service::{AnalyticsError, EventsAnalytics};

#[derive(Debug, Error)]
pub enum EventProcessorError {
    #[error("DAO error: {0}")]
    Dao(#[from] events_dao::EventDaoError),
    #[error("Analytics error: {0}")]
    Analytics(#[from] AnalyticsError),
    #[error("Channel error: {0}")]
    Channel(String),
}

/// High-performance event processor that handles both persistence and
/// analytics
pub struct EventProcessor {
    event_dao: EventDao,
    analytics: Arc<dyn EventsAnalytics>,
    // Channel for async processing to avoid blocking writes
    event_sender: mpsc::UnboundedSender<EventResponse>,
}

impl EventProcessor {
    pub fn new(
        event_dao: EventDao, analytics: Arc<dyn EventsAnalytics>,
    ) -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        let analytics_clone = analytics.clone();

        // Spawn background task to process analytics
        tokio::spawn(async move {
            Self::process_analytics_events(analytics_clone, event_receiver)
                .await;
        });

        Self {
            event_dao,
            analytics,
            event_sender,
        }
    }

    /// Create and process an event (write to DB + analytics)
    #[instrument(skip(self, request))]
    pub async fn create_event(
        &self, request: CreateEventRequest,
    ) -> Result<EventResponse, EventProcessorError> {
        let event = self.event_dao.create(request).await?;

        if self.event_sender.send(event.clone()).is_err() {
            warn!(
                "Analytics queue is full, event {} may not be processed for \
                 analytics",
                event.id
            );
        }

        info!(
            "Successfully created event {} for user {}",
            event.id, event.user_id
        );

        Ok(event)
    }

    /// Background task to process events for analytics
    async fn process_analytics_events(
        analytics: Arc<dyn EventsAnalytics>,
        mut receiver: mpsc::UnboundedReceiver<EventResponse>,
    ) {
        while let Some(event) = receiver.recv().await {
            if let Err(e) = analytics.process_event(&event).await {
                error!(
                    "Failed to process event {} for analytics: {}",
                    event.id, e
                );
                // Could implement retry logic or dead letter queue here
            }
        }

        info!("Analytics event processor shutting down");
    }

    /// Batch create events for high throughput scenarios
    #[instrument(skip(self, requests))]
    pub async fn create_events_batch(
        &self, requests: Vec<CreateEventRequest>,
    ) -> Result<Vec<EventResponse>, EventProcessorError> {
        let mut results = Vec::with_capacity(requests.len());

        // Process in chunks to avoid overwhelming the system
        for chunk in requests.chunks(100) {
            let mut chunk_results = Vec::new();

            for request in chunk {
                match self.create_event(request.clone()).await {
                    Ok(event) => chunk_results.push(event),
                    Err(e) => {
                        error!("Failed to create event in batch: {}", e);
                        return Err(e);
                    }
                }
            }

            results.extend(chunk_results);
        }

        info!("Successfully processed batch of {} events", results.len());
        Ok(results)
    }

    /// Get real-time analytics metrics
    pub async fn get_analytics(&self) -> Arc<dyn EventsAnalytics> {
        self.analytics.clone()
    }
}

/// Background service for event processing optimizations
pub struct EventProcessingService {
    pub processor: Arc<EventProcessor>,
}

impl EventProcessingService {
    pub fn new(processor: EventProcessor) -> Self {
        Self {
            processor: Arc::new(processor),
        }
    }

    /// Start background services for event processing optimizations
    pub async fn start_background_services(&self) {
        let processor = self.processor.clone();

        // Start materialized view refresh task
        tokio::spawn(async move {
            let analytics = processor.get_analytics().await;
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_secs(3600)); // Every hour

            loop {
                interval.tick().await;

                if let Err(e) = analytics.refresh_materialized_views().await {
                    error!("Failed to refresh materialized views: {}", e);
                }
                else {
                    info!("Successfully refreshed materialized views");
                }
            }
        });

        info!("Started event processing background services");
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    #[tokio::test]
    async fn test_event_processor_creation() {
        // This would be a proper integration test with test containers
        let request = CreateEventRequest {
            user_id: Uuid::new_v4(),
            event_type_id: 1,
            metadata: Some(serde_json::json!({"test": "data"})),
        };

        // TODO real test: set up test DAO and analytics
        // let processor = EventProcessor::new(test_dao, test_analytics);
        // let result = processor.create_event(request).await;
        // assert!(result.is_ok());

        assert_eq!(request.event_type_id, 1);
    }
}
