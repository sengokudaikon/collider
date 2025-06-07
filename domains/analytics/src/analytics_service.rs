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
    // Real-time aggregations
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

    // Complex queries via materialized views
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

    // Background maintenance
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
        // Convert EventResponse to EventAggregation
        // Note: We need to convert event_type_id back to a string for Redis
        // keys
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
        // Convert event_type_ids to strings for materialized view
        // compatibility
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
        // Refresh all materialized views
        tokio::try_join!(
            self.view_manager.refresh_hourly_summaries(),
            self.view_manager.refresh_daily_user_activity(),
            self.view_manager.refresh_popular_events()
        )?;

        info!("Successfully refreshed all materialized views");
        Ok(())
    }
}

// Background task for refreshing materialized views
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
            tokio::time::interval(tokio::time::Duration::from_secs(3600)); // Every hour

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

    // More frequent refresh for popular events (every 15 minutes)
    #[instrument(skip(self))]
    pub async fn run_frequent_popular_events_refresh(&self) {
        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_secs(900)); // Every 15 minutes

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
    use chrono::Utc;
    use uuid::Uuid;

    use super::*;

    #[tokio::test]
    async fn test_analytics_flow() {
        // This would be a real integration test with test containers
        // For now, just testing the structure

        let event = EventResponse {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            event_type_id: 1,
            timestamp: Utc::now(),
            metadata: Some(serde_json::json!({"page": "home"})),
        };

        // TODO real test, you'd set up test Redis and Postgres instances
        // and verify that the analytics pipeline processes events correctly
        assert_eq!(event.event_type_id, 1);
    }
}
