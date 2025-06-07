use analytics::{TimeBucket, aggregations::RedisEventAggregator};
use chrono::{DateTime, Duration, Utc};
use redis::AsyncCommands;
use redis_connection::connection::RedisConnectionManager;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::instrument;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum UserAnalyticsError {
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("Redis pool error: {0}")]
    Pool(#[from] redis_connection::PoolError),
    #[error("Analytics error: {0}")]
    Analytics(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserEventMetrics {
    pub total_events: u64,
    pub events_last_24h: u64,
    pub events_last_7d: u64,
    pub events_last_30d: u64,
    pub most_frequent_event_type: Option<String>,
    pub event_type_counts: Vec<EventTypeCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventTypeCount {
    pub event_type: String,
    pub count: u64,
}

#[derive(Clone, Default)]
pub struct UserAnalyticsService {
    aggregator: RedisEventAggregator,
}

impl UserAnalyticsService {
    pub fn new() -> Self {
        Self {
            aggregator: RedisEventAggregator::new(),
        }
    }

    #[instrument(skip(self))]
    pub async fn get_user_metrics(
        &self, user_id: Uuid,
    ) -> Result<UserEventMetrics, UserAnalyticsError> {
        let now = Utc::now();

        // Get metrics for different time periods
        let events_last_24h = self
            .get_user_events_in_period(
                user_id,
                now - Duration::hours(24),
                now,
            )
            .await?;
        let events_last_7d = self
            .get_user_events_in_period(user_id, now - Duration::days(7), now)
            .await?;
        let events_last_30d = self
            .get_user_events_in_period(user_id, now - Duration::days(30), now)
            .await?;

        // Get total events from monthly buckets (approximation for
        // efficiency)
        let total_events = self.get_user_total_events(user_id).await?;

        // Get event type breakdown for last 30 days
        let event_type_counts = self
            .get_user_event_types(user_id, now - Duration::days(30), now)
            .await?;

        let most_frequent_event_type = event_type_counts
            .iter()
            .max_by_key(|etc| etc.count)
            .map(|etc| etc.event_type.clone());

        Ok(UserEventMetrics {
            total_events,
            events_last_24h,
            events_last_7d,
            events_last_30d,
            most_frequent_event_type,
            event_type_counts,
        })
    }

    #[instrument(skip(self))]
    pub async fn get_batch_user_metrics(
        &self, user_ids: Vec<Uuid>,
    ) -> Result<Vec<(Uuid, UserEventMetrics)>, UserAnalyticsError> {
        use futures::future::join_all;

        // Use concurrent futures for better performance than sequential Redis
        // calls
        let futures = user_ids.iter().map(|&user_id| {
            async move {
                match self.get_user_metrics(user_id).await {
                    Ok(metrics) => Some((user_id, metrics)),
                    Err(e) => {
                        tracing::warn!(
                            "Failed to get metrics for user {}: {}",
                            user_id,
                            e
                        );
                        None
                    }
                }
            }
        });

        let results = join_all(futures)
            .await
            .into_iter()
            .flatten()
            .collect();

        Ok(results)
    }

    async fn get_user_events_in_period(
        &self, user_id: Uuid, start: DateTime<Utc>, end: DateTime<Utc>,
    ) -> Result<u64, UserAnalyticsError> {
        let redis = RedisConnectionManager::from_static();
        let mut conn = redis
            .get_mut_connection()
            .await
            .map_err(UserAnalyticsError::from)?;

        // Use appropriate bucket based on time range
        let bucket = if end - start <= Duration::days(1) {
            TimeBucket::Hour
        }
        else if end - start <= Duration::days(7) {
            TimeBucket::Day
        }
        else {
            TimeBucket::Week
        };

        let mut total = 0u64;
        let mut current = start;

        while current < end {
            let bucket_key = self.aggregator.build_bucket_key(
                bucket,
                current,
                Some(user_id.to_string().as_str()),
            );
            let key = format!("{}:total", bucket_key);

            let count: u64 = conn.get(&key).await.unwrap_or(0);
            total += count;

            current += match bucket {
                TimeBucket::Hour => Duration::hours(1),
                TimeBucket::Day => Duration::days(1),
                TimeBucket::Week => Duration::weeks(1),
                _ => Duration::days(1),
            };
        }

        Ok(total)
    }

    async fn get_user_total_events(
        &self, user_id: Uuid,
    ) -> Result<u64, UserAnalyticsError> {
        let redis = RedisConnectionManager::from_static();
        let mut conn = redis
            .get_mut_connection()
            .await
            .map_err(UserAnalyticsError::from)?;

        // Get pattern for all monthly buckets for this user
        let pattern = format!("events:month:*:users:{}:total", user_id);
        let keys: Vec<String> = conn.keys(&pattern).await?;

        let mut total = 0u64;
        for key in keys {
            let count: u64 = conn.get(&key).await.unwrap_or(0);
            total += count;
        }

        Ok(total)
    }

    async fn get_user_event_types(
        &self, user_id: Uuid, start: DateTime<Utc>, end: DateTime<Utc>,
    ) -> Result<Vec<EventTypeCount>, UserAnalyticsError> {
        let redis = RedisConnectionManager::from_static();
        let mut conn = redis
            .get_mut_connection()
            .await
            .map_err(UserAnalyticsError::from)?;

        let mut type_counts = std::collections::HashMap::<String, u64>::new();
        let mut current = start;

        while current < end {
            let bucket_key = self.aggregator.build_bucket_key(
                TimeBucket::Day,
                current,
                Some(user_id.to_string().as_str()),
            );
            let pattern = format!("{}:types:*", bucket_key);
            let keys: Vec<String> = conn.keys(&pattern).await?;

            for key in keys {
                if let Some(event_type) = key.split(':').next_back() {
                    let count: u64 = conn.get(&key).await.unwrap_or(0);
                    *type_counts
                        .entry(event_type.to_string())
                        .or_insert(0) += count;
                }
            }

            current += Duration::days(1);
        }

        let mut result: Vec<EventTypeCount> = type_counts
            .into_iter()
            .map(|(event_type, count)| EventTypeCount { event_type, count })
            .collect();

        // Sort by count descending
        result.sort_by(|a, b| b.count.cmp(&a.count));

        Ok(result)
    }
}
