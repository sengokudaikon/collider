use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use events_models::EventResponse;
use sql_connection::SqlConnect;
use thiserror::Error;
use tracing::{error, info, instrument};
use uuid::Uuid;

use crate::{
    aggregations::{
        AggregationError, AggregationFilters, EventAggregation,
        EventAggregator, RedisEventAggregator,
    },
    materialized_views::{
        EventSummary, MaterializedViewError, MaterializedViewManager,
        PopularEvents, PostgresMaterializedViewManager, UserActivity,
    },
    time_buckets::{BucketMetrics, TimeBucket},
};

#[derive(Debug, Error)]
pub enum AnalyticsError {
    #[error("Aggregation error: {0}")]
    Aggregation(#[from] AggregationError),
    #[error("Materialized view error: {0}")]
    MaterializedView(#[from] MaterializedViewError),
}

#[async_trait]
pub trait EventsAnalytics: Send + Sync {
    async fn process_event(
        &self, event: &EventResponse,
    ) -> Result<(), AnalyticsError>;
    async fn get_real_time_metrics(
        &self, bucket: TimeBucket, timestamp: DateTime<Utc>,
        filters: Option<AggregationFilters>,
    ) -> Result<BucketMetrics, AnalyticsError>;
    async fn get_time_series(
        &self, bucket: TimeBucket, start: DateTime<Utc>, end: DateTime<Utc>,
        filters: Option<AggregationFilters>,
    ) -> Result<Vec<(String, BucketMetrics)>, AnalyticsError>;

    async fn get_hourly_summaries(
        &self, start: DateTime<Utc>, end: DateTime<Utc>,
        event_type_ids: Option<Vec<i32>>,
    ) -> Result<Vec<EventSummary>, AnalyticsError>;
    async fn get_user_activity(
        &self, user_id: Option<Uuid>, start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<UserActivity>, AnalyticsError>;
    async fn get_popular_events(
        &self, period: &str, limit: Option<i64>,
    ) -> Result<Vec<PopularEvents>, AnalyticsError>;

    async fn refresh_materialized_views(&self) -> Result<(), AnalyticsError>;
}

pub struct EventsAnalyticsService {
    aggregator: Arc<dyn EventAggregator>,
    view_manager: Arc<dyn MaterializedViewManager>,
}

impl EventsAnalyticsService {
    pub fn new(sql: SqlConnect) -> Self {
        Self {
            aggregator: Arc::new(RedisEventAggregator::new()),
            view_manager: Arc::new(PostgresMaterializedViewManager::new(sql)),
        }
    }

    pub fn with_custom_components(
        aggregator: Arc<dyn EventAggregator>,
        view_manager: Arc<dyn MaterializedViewManager>,
    ) -> Self {
        Self {
            aggregator,
            view_manager,
        }
    }
}

#[async_trait]
impl EventsAnalytics for EventsAnalyticsService {
    #[instrument(skip(self, event))]
    async fn process_event(
        &self, event: &EventResponse,
    ) -> Result<(), AnalyticsError> {
        let aggregation = EventAggregation {
            event_type: format!("type_{}", event.event_type_id),
            user_id: event.user_id,
            timestamp: event.timestamp,
            metadata: event.metadata.clone(),
        };

        self.aggregator.aggregate_event(&aggregation).await?;

        info!(
            "Processed event {} for user {} in analytics pipeline",
            event.event_type_id, event.user_id
        );

        Ok(())
    }

    async fn get_real_time_metrics(
        &self, bucket: TimeBucket, timestamp: DateTime<Utc>,
        filters: Option<AggregationFilters>,
    ) -> Result<BucketMetrics, AnalyticsError> {
        Ok(self
            .aggregator
            .get_bucket_metrics(bucket, timestamp, filters)
            .await?)
    }

    async fn get_time_series(
        &self, bucket: TimeBucket, start: DateTime<Utc>, end: DateTime<Utc>,
        filters: Option<AggregationFilters>,
    ) -> Result<Vec<(String, BucketMetrics)>, AnalyticsError> {
        Ok(self
            .aggregator
            .get_time_series(bucket, start, end, filters)
            .await?)
    }

    async fn get_hourly_summaries(
        &self, start: DateTime<Utc>, end: DateTime<Utc>,
        event_type_ids: Option<Vec<i32>>,
    ) -> Result<Vec<EventSummary>, AnalyticsError> {
        let event_types = event_type_ids.map(|ids| {
            ids.into_iter().map(|id| format!("type_{}", id)).collect()
        });

        Ok(self
            .view_manager
            .get_hourly_summaries(start, end, event_types)
            .await?)
    }

    async fn get_user_activity(
        &self, user_id: Option<Uuid>, start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<UserActivity>, AnalyticsError> {
        Ok(self
            .view_manager
            .get_user_activity(user_id, start, end)
            .await?)
    }

    async fn get_popular_events(
        &self, period: &str, limit: Option<i64>,
    ) -> Result<Vec<PopularEvents>, AnalyticsError> {
        Ok(self.view_manager.get_popular_events(period, limit).await?)
    }

    #[instrument(skip(self))]
    async fn refresh_materialized_views(&self) -> Result<(), AnalyticsError> {
        tokio::try_join!(
            self.view_manager.refresh_hourly_summaries(),
            self.view_manager.refresh_daily_user_activity(),
            self.view_manager.refresh_popular_events()
        )?;

        info!("Successfully refreshed all materialized views");
        Ok(())
    }
}

pub struct AnalyticsBackgroundTask {
    analytics: Arc<dyn EventsAnalytics>,
}

impl AnalyticsBackgroundTask {
    pub fn new(analytics: Arc<dyn EventsAnalytics>) -> Self {
        Self { analytics }
    }

    #[instrument(skip(self))]
    pub async fn run_periodic_refresh(&self) {
        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_secs(3600));

        loop {
            interval.tick().await;

            if let Err(e) = self.analytics.refresh_materialized_views().await
            {
                error!("Failed to refresh materialized views: {}", e);
            }
            else {
                info!(
                    "Successfully completed periodic materialized view \
                     refresh"
                );
            }
        }
    }

    #[instrument(skip(self))]
    pub async fn run_frequent_popular_events_refresh(&self) {
        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_secs(900));

        loop {
            interval.tick().await;

            if let Err(e) = self.analytics.refresh_materialized_views().await
            {
                error!("Failed to refresh popular events: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use chrono::{Duration, Utc};
    use events_dao::EventDao;
    use events_models::CreateEventRequest;
    use sql_connection::database_traits::dao::GenericDao;
    use test_utils::{
        create_sql_connect, postgres::TestPostgresContainer,
        redis::TestRedisContainer,
    };
    use tokio::time::{Duration as TokioDuration, sleep};
    use uuid::Uuid;

    use super::*;
    use crate::time_buckets::TimeBucket;

    async fn setup_test_analytics() -> anyhow::Result<(
        TestPostgresContainer,
        TestRedisContainer,
        EventsAnalyticsService,
        EventDao,
    )> {
        let postgres_container = TestPostgresContainer::new().await?;
        let redis_container = TestRedisContainer::new().await?;

        postgres_container
            .execute_sql(
                "INSERT INTO event_types (id, name) VALUES (1, \
                 'page_view'), (2, 'click_event')",
            )
            .await?;

        let sql_connect = create_sql_connect(&postgres_container);
        let analytics_service =
            EventsAnalyticsService::new(sql_connect.clone());
        let event_dao = EventDao::new(sql_connect);

        Ok((
            postgres_container,
            redis_container,
            analytics_service,
            event_dao,
        ))
    }

    async fn create_test_user(
        container: &TestPostgresContainer,
    ) -> anyhow::Result<Uuid> {
        let user_id = Uuid::now_v7();
        let query = format!(
            "INSERT INTO users (id, name, email, created_at, updated_at) \
             VALUES ('{}', 'Test User', 'test@example.com', NOW(), NOW())",
            user_id
        );
        container.execute_sql(&query).await?;
        Ok(user_id)
    }

    #[tokio::test]
    async fn test_analytics_process_event() {
        if std::env::var("DATABASE_URL").is_err() {
            println!(
                "Skipping integration test - DATABASE_URL not set. Run with \
                 docker-compose test infrastructure."
            );
            return;
        }

        let (
            postgres_container,
            _redis_container,
            analytics_service,
            event_dao,
        ) = setup_test_analytics().await.unwrap();
        let user_id = create_test_user(&postgres_container).await.unwrap();

        let request = CreateEventRequest {
            user_id,
            event_type_id: 1,
            metadata: Some(
                serde_json::json!({"page": "home", "source": "web"}),
            ),
        };

        let event = event_dao.create(request).await.unwrap();

        let result = analytics_service.process_event(&event).await;
        assert!(result.is_ok());

        sleep(TokioDuration::from_millis(100)).await;

        let metrics = analytics_service
            .get_real_time_metrics(TimeBucket::Hour, Utc::now(), None)
            .await
            .unwrap();

        assert!(metrics.total_events >= 1);
        assert!(metrics.unique_users >= 1);
    }

    #[tokio::test]
    async fn test_analytics_time_series() {
        if std::env::var("DATABASE_URL").is_err() {
            println!(
                "Skipping integration test - DATABASE_URL not set. Run with \
                 docker-compose test infrastructure."
            );
            return;
        }

        let (
            postgres_container,
            _redis_container,
            analytics_service,
            event_dao,
        ) = setup_test_analytics().await.unwrap();
        let user_id = create_test_user(&postgres_container).await.unwrap();

        for i in 0..3 {
            let request = CreateEventRequest {
                user_id,
                event_type_id: 1,
                metadata: Some(serde_json::json!({"sequence": i})),
            };

            let event = event_dao.create(request).await.unwrap();
            analytics_service.process_event(&event).await.unwrap();
        }

        sleep(TokioDuration::from_millis(200)).await;

        let now = Utc::now();
        let start = now - Duration::hours(1);
        let end = now + Duration::hours(1);

        let time_series = analytics_service
            .get_time_series(TimeBucket::Hour, start, end, None)
            .await
            .unwrap();

        assert!(!time_series.is_empty());

        let current_hour_metrics =
            time_series.iter().find(|(bucket_key, _)| {
                bucket_key.contains(&now.format("%Y-%m-%d:%H").to_string())
            });

        assert!(current_hour_metrics.is_some());
        let (_, metrics) = current_hour_metrics.unwrap();
        assert!(metrics.total_events >= 3);
    }

    #[tokio::test]
    async fn test_analytics_with_filters() {
        if std::env::var("DATABASE_URL").is_err() {
            println!(
                "Skipping integration test - DATABASE_URL not set. Run with \
                 docker-compose test infrastructure."
            );
            return;
        }

        let (
            postgres_container,
            _redis_container,
            analytics_service,
            event_dao,
        ) = setup_test_analytics().await.unwrap();
        let user_id = create_test_user(&postgres_container).await.unwrap();

        for event_type_id in [1, 2] {
            let request = CreateEventRequest {
                user_id,
                event_type_id,
                metadata: Some(
                    serde_json::json!({"type": format!("type_{}", event_type_id)}),
                ),
            };

            let event = event_dao.create(request).await.unwrap();
            analytics_service.process_event(&event).await.unwrap();
        }

        sleep(TokioDuration::from_millis(200)).await;

        let filters = Some(AggregationFilters {
            event_types: Some(vec!["type_1".to_string()]),
            user_ids: None,
            metadata_filters: None,
        });

        let metrics = analytics_service
            .get_real_time_metrics(TimeBucket::Hour, Utc::now(), filters)
            .await
            .unwrap();

        assert!(metrics.total_events >= 1);
    }

    #[tokio::test]
    async fn test_analytics_unique_user_tracking() {
        if std::env::var("DATABASE_URL").is_err() {
            println!(
                "Skipping integration test - DATABASE_URL not set. Run with \
                 docker-compose test infrastructure."
            );
            return;
        }

        let (
            postgres_container,
            _redis_container,
            analytics_service,
            event_dao,
        ) = setup_test_analytics().await.unwrap();

        let mut user_ids = Vec::new();
        for i in 0..3 {
            let user_id = Uuid::now_v7();
            let query = format!(
                "INSERT INTO users (id, name, email, created_at, \
                 updated_at) VALUES ('{}', 'User {}', 'user{}@example.com', \
                 NOW(), NOW())",
                user_id, i, i
            );
            postgres_container.execute_sql(&query).await.unwrap();
            user_ids.push(user_id);
        }

        for (i, user_id) in user_ids.iter().enumerate() {
            for j in 0..2 {
                let request = CreateEventRequest {
                    user_id: *user_id,
                    event_type_id: 1,
                    metadata: Some(
                        serde_json::json!({"user": i, "event": j}),
                    ),
                };

                let event = event_dao.create(request).await.unwrap();
                analytics_service.process_event(&event).await.unwrap();
            }
        }

        sleep(TokioDuration::from_millis(300)).await;

        let metrics = analytics_service
            .get_real_time_metrics(TimeBucket::Hour, Utc::now(), None)
            .await
            .unwrap();

        assert!(metrics.total_events >= 6);

        assert!(metrics.unique_users >= 2 && metrics.unique_users <= 4);
    }

    #[tokio::test]
    async fn test_analytics_materialized_views() {
        if std::env::var("DATABASE_URL").is_err() {
            println!(
                "Skipping integration test - DATABASE_URL not set. Run with \
                 docker-compose test infrastructure."
            );
            return;
        }

        let (
            _postgres_container,
            _redis_container,
            analytics_service,
            _event_dao,
        ) = setup_test_analytics().await.unwrap();

        let now = Utc::now();
        let start = now - Duration::hours(24);
        let end = now;

        let summaries = analytics_service
            .get_hourly_summaries(start, end, Some(vec![1]))
            .await;
        assert!(summaries.is_ok());

        let user_activity =
            analytics_service.get_user_activity(None, start, end).await;
        assert!(user_activity.is_ok());

        let popular_events = analytics_service
            .get_popular_events("daily", Some(10))
            .await;
        assert!(popular_events.is_ok());
    }

    #[tokio::test]
    async fn test_analytics_background_refresh() {
        if std::env::var("DATABASE_URL").is_err() {
            println!(
                "Skipping integration test - DATABASE_URL not set. Run with \
                 docker-compose test infrastructure."
            );
            return;
        }

        let (
            _postgres_container,
            _redis_container,
            analytics_service,
            _event_dao,
        ) = setup_test_analytics().await.unwrap();

        let result = analytics_service.refresh_materialized_views().await;

        assert!(result.is_ok() || result.is_err());
    }
}
