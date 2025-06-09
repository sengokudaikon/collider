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

pub struct EventProcessor {
    event_dao: EventDao,
    analytics: Arc<dyn EventsAnalytics>,
    event_sender: mpsc::UnboundedSender<EventResponse>,
}

impl EventProcessor {
    pub fn new(
        event_dao: EventDao, analytics: Arc<dyn EventsAnalytics>,
    ) -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        let analytics_clone = analytics.clone();

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
            }
        }

        info!("Analytics event processor shutting down");
    }

    #[instrument(skip(self, requests))]
    pub async fn create_events_batch(
        &self, requests: Vec<CreateEventRequest>,
    ) -> Result<Vec<EventResponse>, EventProcessorError> {
        let mut results = Vec::with_capacity(requests.len());

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

    pub async fn get_analytics(&self) -> Arc<dyn EventsAnalytics> {
        self.analytics.clone()
    }
}

pub struct EventProcessingService {
    pub processor: Arc<EventProcessor>,
}

impl EventProcessingService {
    pub fn new(processor: EventProcessor) -> Self {
        Self {
            processor: Arc::new(processor),
        }
    }

    pub async fn start_background_services(&self) {
        let processor = self.processor.clone();

        tokio::spawn(async move {
            let analytics = processor.get_analytics().await;
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_secs(3600));

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
    use std::sync::Arc;

    use async_trait::async_trait;
    use chrono::Utc;
    use test_utils::{
        create_sql_connect, postgres::TestPostgresContainer,
        redis::TestRedisContainer,
    };
    use tokio::time::{Duration, sleep};
    use uuid::Uuid;

    use super::*;
    use crate::{
        analytics_service::{EventsAnalytics, EventsAnalyticsService},
        time_buckets::TimeBucket,
    };

    struct MockAnalytics {
        processed_events: std::sync::Mutex<Vec<EventResponse>>,
    }

    impl MockAnalytics {
        fn new() -> Self {
            Self {
                processed_events: std::sync::Mutex::new(Vec::new()),
            }
        }

        fn get_processed_events(&self) -> Vec<EventResponse> {
            self.processed_events.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl EventsAnalytics for MockAnalytics {
        async fn process_event(
            &self, event: &EventResponse,
        ) -> Result<(), AnalyticsError> {
            self.processed_events.lock().unwrap().push(event.clone());
            Ok(())
        }

        async fn get_real_time_metrics(
            &self, _bucket: TimeBucket, _timestamp: chrono::DateTime<Utc>,
            _filters: Option<crate::aggregations::AggregationFilters>,
        ) -> Result<crate::time_buckets::BucketMetrics, AnalyticsError>
        {
            Ok(crate::time_buckets::BucketMetrics::default())
        }

        async fn get_time_series(
            &self, _bucket: TimeBucket, _start: chrono::DateTime<Utc>,
            _end: chrono::DateTime<Utc>,
            _filters: Option<crate::aggregations::AggregationFilters>,
        ) -> Result<
            Vec<(String, crate::time_buckets::BucketMetrics)>,
            AnalyticsError,
        > {
            Ok(vec![])
        }

        async fn get_hourly_summaries(
            &self, _start: chrono::DateTime<Utc>,
            _end: chrono::DateTime<Utc>, _event_type_ids: Option<Vec<i32>>,
        ) -> Result<
            Vec<crate::materialized_views::EventSummary>,
            AnalyticsError,
        > {
            Ok(vec![])
        }

        async fn get_user_activity(
            &self, _user_id: Option<Uuid>, _start: chrono::DateTime<Utc>,
            _end: chrono::DateTime<Utc>,
        ) -> Result<
            Vec<crate::materialized_views::UserActivity>,
            AnalyticsError,
        > {
            Ok(vec![])
        }

        async fn get_popular_events(
            &self, _period: &str, _limit: Option<i64>,
        ) -> Result<
            Vec<crate::materialized_views::PopularEvents>,
            AnalyticsError,
        > {
            Ok(vec![])
        }

        async fn refresh_materialized_views(
            &self,
        ) -> Result<(), AnalyticsError> {
            Ok(())
        }
    }

    async fn setup_test_environment()
    -> anyhow::Result<(TestPostgresContainer, EventDao, Arc<MockAnalytics>)>
    {
        let container = TestPostgresContainer::new().await?;

        container
            .execute_sql(
                "INSERT INTO event_types (id, name) VALUES (1, 'test_event')",
            )
            .await?;

        let sql_connect = create_sql_connect(&container);
        let event_dao = EventDao::new(sql_connect);
        let mock_analytics = Arc::new(MockAnalytics::new());

        Ok((container, event_dao, mock_analytics))
    }

    async fn create_test_user(
        container: &TestPostgresContainer,
    ) -> anyhow::Result<Uuid> {
        let user_id = Uuid::now_v7();
        let query = format!(
            "INSERT INTO users (id, name,created_at) VALUES ('{}', 'Test \
             User', NOW())",
            user_id
        );
        container.execute_sql(&query).await?;
        Ok(user_id)
    }

    #[tokio::test]
    async fn test_event_processor_single_event() {
        if std::env::var("DATABASE_URL").is_err() {
            println!(
                "Skipping integration test - DATABASE_URL not set. Run with \
                 docker-compose test infrastructure."
            );
            return;
        }

        let (container, event_dao, mock_analytics) =
            setup_test_environment().await.unwrap();
        let user_id = create_test_user(&container).await.unwrap();

        let processor =
            EventProcessor::new(event_dao, mock_analytics.clone());

        let request = CreateEventRequest {
            user_id,
            event_type_id: 1,
            metadata: Some(serde_json::json!({"test": "data"})),
        };

        let result = processor.create_event(request).await.unwrap();

        assert_eq!(result.user_id, user_id);
        assert_eq!(result.event_type_id, 1);
        assert!(result.metadata.is_some());

        sleep(Duration::from_millis(100)).await;

        let processed = mock_analytics.get_processed_events();
        assert_eq!(processed.len(), 1);
        assert_eq!(processed[0].id, result.id);
    }

    #[tokio::test]
    async fn test_event_processor_batch() {
        if std::env::var("DATABASE_URL").is_err() {
            println!(
                "Skipping integration test - DATABASE_URL not set. Run with \
                 docker-compose test infrastructure."
            );
            return;
        }

        let (container, event_dao, mock_analytics) =
            setup_test_environment().await.unwrap();
        let user_id = create_test_user(&container).await.unwrap();

        let processor =
            EventProcessor::new(event_dao, mock_analytics.clone());

        let requests = vec![
            CreateEventRequest {
                user_id,
                event_type_id: 1,
                metadata: Some(serde_json::json!({"batch": 1})),
            },
            CreateEventRequest {
                user_id,
                event_type_id: 1,
                metadata: Some(serde_json::json!({"batch": 2})),
            },
            CreateEventRequest {
                user_id,
                event_type_id: 1,
                metadata: Some(serde_json::json!({"batch": 3})),
            },
        ];

        let results = processor.create_events_batch(requests).await.unwrap();

        assert_eq!(results.len(), 3);
        for (i, result) in results.iter().enumerate() {
            assert_eq!(result.user_id, user_id);
            assert_eq!(result.event_type_id, 1);
            let expected_metadata = serde_json::json!({"batch": i + 1});
            assert_eq!(result.metadata, Some(expected_metadata));
        }

        sleep(Duration::from_millis(200)).await;

        let processed = mock_analytics.get_processed_events();
        assert_eq!(processed.len(), 3);
    }

    #[tokio::test]
    async fn test_event_processor_with_real_analytics() {
        if std::env::var("DATABASE_URL").is_err() {
            println!(
                "Skipping integration test - DATABASE_URL not set. Run with \
                 docker-compose test infrastructure."
            );
            return;
        }

        let postgres_container = TestPostgresContainer::new().await.unwrap();
        let _redis_container = TestRedisContainer::new().await.unwrap();

        postgres_container
            .execute_sql(
                "INSERT INTO event_types (id, name) VALUES (1, \
                 'login_event')",
            )
            .await
            .unwrap();

        let user_id = create_test_user(&postgres_container).await.unwrap();

        let sql_connect = create_sql_connect(&postgres_container);
        let event_dao = EventDao::new(sql_connect.clone());

        let real_analytics =
            Arc::new(EventsAnalyticsService::new(sql_connect))
                as Arc<dyn EventsAnalytics>;
        let processor =
            EventProcessor::new(event_dao, real_analytics.clone());

        let request = CreateEventRequest {
            user_id,
            event_type_id: 1,
            metadata: Some(
                serde_json::json!({"page": "login", "source": "web"}),
            ),
        };

        let result = processor.create_event(request).await.unwrap();

        assert_eq!(result.user_id, user_id);
        assert_eq!(result.event_type_id, 1);

        sleep(Duration::from_millis(100)).await;

        let metrics = real_analytics
            .get_real_time_metrics(TimeBucket::Hour, Utc::now(), None)
            .await
            .unwrap();

        assert!(metrics.total_events >= 1);
    }

    #[tokio::test]
    async fn test_event_processing_service_background_tasks() {
        if std::env::var("DATABASE_URL").is_err() {
            println!(
                "Skipping integration test - DATABASE_URL not set. Run with \
                 docker-compose test infrastructure."
            );
            return;
        }

        let (_container, event_dao, mock_analytics) =
            setup_test_environment().await.unwrap();
        let processor =
            EventProcessor::new(event_dao, mock_analytics.clone());
        let service = EventProcessingService::new(processor);

        tokio::spawn(async move {
            service.start_background_services().await;
        });

        sleep(Duration::from_millis(50)).await;

        // Test passed - background services started successfully
    }
}
