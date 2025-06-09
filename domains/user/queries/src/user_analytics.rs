use std::time::Duration as StdDuration;

use analytics::{TimeBucket, aggregations::RedisEventAggregator};
use chrono::{DateTime, Duration, Utc};
use redis::AsyncCommands;
use redis_connection::{
    connection::RedisConnectionManager, json::Json, type_bind::RedisTypeBind,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::instrument;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::cache_keys::*;

#[derive(Debug, Error)]
pub enum UserAnalyticsError {
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("Redis pool error: {0}")]
    Pool(#[from] redis_connection::PoolError),
    #[error("Analytics error: {0}")]
    Analytics(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserEventMetrics {
    pub total_events: u64,
    pub events_last_24h: u64,
    pub events_last_7d: u64,
    pub events_last_30d: u64,
    pub most_frequent_event_type: Option<String>,
    pub event_type_counts: Vec<EventTypeCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
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
        let redis = RedisConnectionManager::from_static();
        let mut conn = redis.get_connection().await?;

        // Try to get from cache first
        let cache_key = UserMetricsCacheKey;
        let mut cache = cache_key.bind_with(&mut *conn, &user_id);

        if let Ok(Some(metrics)) = cache.try_get().await {
            tracing::debug!("Cache hit for user {} metrics", user_id);
            return Ok(metrics.inner());
        }

        tracing::debug!(
            "Cache miss for user {} metrics, computing...",
            user_id
        );
        let now = Utc::now();

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

        let total_events = self.get_user_total_events(user_id).await?;

        let event_type_counts = self
            .get_user_event_types(user_id, now - Duration::days(30), now)
            .await?;

        let most_frequent_event_type = event_type_counts
            .iter()
            .max_by_key(|etc| etc.count)
            .map(|etc| etc.event_type.clone());

        let metrics = UserEventMetrics {
            total_events,
            events_last_24h,
            events_last_7d,
            events_last_30d,
            most_frequent_event_type,
            event_type_counts,
        };

        // Cache the result for only 30 seconds since metrics update in
        // real-time
        let _ = cache
            .set_with_expire::<()>(
                Json(metrics.clone()).serde().unwrap(),
                StdDuration::from_secs(30),
            )
            .await;

        Ok(metrics)
    }

    #[instrument(skip(self))]
    pub async fn get_batch_user_metrics(
        &self, user_ids: Vec<Uuid>,
    ) -> Result<Vec<(Uuid, UserEventMetrics)>, UserAnalyticsError> {
        use futures::future::join_all;

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

        let results = join_all(futures).await.into_iter().flatten().collect();

        Ok(results)
    }

    async fn get_user_events_in_period(
        &self, user_id: Uuid, start: DateTime<Utc>, end: DateTime<Utc>,
    ) -> Result<u64, UserAnalyticsError> {
        let redis = RedisConnectionManager::from_static();
        let mut conn = redis
            .get_connection()
            .await
            .map_err(UserAnalyticsError::from)?;

        // Create cache key based on period
        let period = format!("{}-{}", start.timestamp(), end.timestamp());
        let bucket_type = if end - start <= Duration::days(1) {
            "hour"
        }
        else if end - start <= Duration::days(7) {
            "day"
        }
        else {
            "week"
        };

        let cache_key = UserEventCountCacheKey;
        let mut cache = cache_key.bind_with_args(
            &mut conn,
            (&user_id, &period.to_string(), &bucket_type.to_string()),
        );

        if let Ok(Some(count)) = cache.try_get().await {
            tracing::debug!(
                "Cache hit for user {} event count period {}",
                user_id,
                period
            );
            return Ok(count.inner());
        }

        let bucket = if end - start <= Duration::days(1) {
            TimeBucket::Hour
        }
        else if end - start <= Duration::days(7) {
            TimeBucket::Day
        }
        else {
            TimeBucket::Week
        };

        // Use separate connection for direct Redis operations to avoid
        // borrowing conflicts
        let mut direct_conn = redis
            .get_connection()
            .await
            .map_err(UserAnalyticsError::from)?;

        let mut total = 0u64;
        let mut current = start;

        while current < end {
            let bucket_key = self.aggregator.build_bucket_key(
                bucket,
                current,
                Some(user_id.to_string().as_str()),
            );
            let key = format!("{}:total", bucket_key);

            // Use direct Redis operation since we're in a loop
            let count: u64 = direct_conn.get(&key).await.unwrap_or(0);
            total += count;

            current += match bucket {
                TimeBucket::Hour => Duration::hours(1),
                TimeBucket::Day => Duration::days(1),
                TimeBucket::Week => Duration::weeks(1),
                _ => Duration::days(1),
            };
        }

        // Cache the result for only 1 minute since event counts update in
        // real-time
        let _ = cache
            .set_with_expire::<()>(
                Json(total).serde().unwrap(),
                StdDuration::from_secs(60),
            )
            .await;

        Ok(total)
    }

    async fn get_user_total_events(
        &self, user_id: Uuid,
    ) -> Result<u64, UserAnalyticsError> {
        let redis = RedisConnectionManager::from_static();
        let mut cache_conn = redis
            .get_connection()
            .await
            .map_err(UserAnalyticsError::from)?;

        // Try cache first
        let cache_key = UserTotalEventsCacheKey;
        let mut cache = cache_key.bind_with(&mut cache_conn, &user_id);

        if let Ok(Some(total)) = cache.try_get().await {
            tracing::debug!("Cache hit for user {} total events", user_id);
            return Ok(total.inner());
        }

        // Use separate connection for direct Redis operations
        let mut direct_conn = redis
            .get_connection()
            .await
            .map_err(UserAnalyticsError::from)?;

        let pattern = format!("events:month:*:users:{}:total", user_id);
        let keys: Vec<String> = direct_conn.keys(&pattern).await?;

        let mut total = 0u64;
        for key in keys {
            let count: u64 = direct_conn.get(&key).await.unwrap_or(0);
            total += count;
        }

        // Cache for only 2 minutes since total events can update frequently
        let _ = cache
            .set_with_expire::<()>(
                Json(total).serde().unwrap(),
                StdDuration::from_secs(120),
            )
            .await;

        Ok(total)
    }

    async fn get_user_event_types(
        &self, user_id: Uuid, start: DateTime<Utc>, end: DateTime<Utc>,
    ) -> Result<Vec<EventTypeCount>, UserAnalyticsError> {
        let redis = RedisConnectionManager::from_static();
        let mut cache_conn = redis
            .get_connection()
            .await
            .map_err(UserAnalyticsError::from)?;

        // Cache key with time range
        let start_str = start.timestamp().to_string();
        let end_str = end.timestamp().to_string();
        let cache_key = UserEventTypesCacheKey;
        let mut cache = cache_key.bind_with_args(
            &mut cache_conn,
            (&user_id, &start_str, &end_str),
        );

        if let Ok(Some(types)) = cache.try_get().await {
            tracing::debug!("Cache hit for user {} event types", user_id);
            return Ok(types.inner());
        }

        // Use separate connection for direct Redis operations
        let mut direct_conn = redis
            .get_connection()
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
            let keys: Vec<String> = direct_conn.keys(&pattern).await?;

            for key in keys {
                if let Some(event_type) = key.split(':').next_back() {
                    let count: u64 = direct_conn.get(&key).await.unwrap_or(0);
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

        result.sort_by(|a, b| b.count.cmp(&a.count));

        // Cache for only 1 minute since event types can change
        let _ = cache
            .set_with_expire::<()>(
                Json(result.clone()).serde().unwrap(),
                StdDuration::from_secs(60),
            )
            .await;

        Ok(result)
    }
}
