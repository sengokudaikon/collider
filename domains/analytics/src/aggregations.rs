use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use redis_connection::{
    AsyncCommands, FromRedisValue, PoolError, RedisError,
    connection::RedisConnectionManager,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, instrument};
use uuid::Uuid;

use crate::time_buckets::{BucketMetrics, TimeBucket};

#[derive(Debug, Error)]
pub enum AggregationError {
    #[error("Redis pool error: {0}")]
    Pool(#[from] PoolError),
    #[error("Redis command error: {0}")]
    Redis(#[from] RedisError),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventAggregation {
    pub event_type: String,
    pub user_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<serde_json::Value>,
}

#[async_trait]
pub trait EventAggregator: Send + Sync {
    async fn aggregate_event(
        &self, event: &EventAggregation,
    ) -> Result<(), AggregationError>;
    async fn get_bucket_metrics(
        &self, bucket: TimeBucket, timestamp: DateTime<Utc>,
        filters: Option<AggregationFilters>,
    ) -> Result<BucketMetrics, AggregationError>;
    async fn get_time_series(
        &self, bucket: TimeBucket, start: DateTime<Utc>, end: DateTime<Utc>,
        filters: Option<AggregationFilters>,
    ) -> Result<Vec<(String, BucketMetrics)>, AggregationError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregationFilters {
    pub event_types: Option<Vec<String>>,
    pub user_ids: Option<Vec<Uuid>>,
    pub metadata_filters: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Clone, Default)]
pub struct RedisEventAggregator;

impl RedisEventAggregator {
    pub fn new() -> Self { Self }

    pub async fn merge_hyperloglog(
        &self, target_key: &str, source_key: &str,
    ) -> Result<(), AggregationError> {
        let redis = RedisConnectionManager::from_static();
        let mut conn = redis.get_mut_connection().await?;
        let _: () = conn.pfmerge(target_key, source_key).await?;
        Ok(())
    }

    pub fn build_bucket_key(
        &self, bucket: TimeBucket, timestamp: DateTime<Utc>,
        suffix: Option<&str>,
    ) -> String {
        let base_key = format!(
            "{}:{}",
            bucket.redis_key_prefix(),
            bucket.bucket_key(timestamp)
        );
        if let Some(suffix) = suffix {
            format!("{}:{}", base_key, suffix)
        }
        else {
            base_key
        }
    }

    async fn increment_counter(
        &self, key: &str, amount: i64,
    ) -> Result<(), AggregationError> {
        let redis = RedisConnectionManager::from_static();
        let mut conn = redis.get_mut_connection().await?;
        let _: () = conn.incr(key, amount).await?;
        Ok(())
    }

    async fn add_to_hyperloglog(
        &self, key: &str, value: &str,
    ) -> Result<(), AggregationError> {
        let redis = RedisConnectionManager::from_static();
        let mut conn = redis.get_mut_connection().await?;
        let _: () = conn.pfadd(key, value).await?;
        Ok(())
    }

    async fn update_hash(
        &self, key: &str, field: &str, value: &str,
    ) -> Result<(), AggregationError> {
        let redis = RedisConnectionManager::from_static();
        let mut conn = redis.get_mut_connection().await?;
        let _: () = conn.hset(key, field, value).await?;
        Ok(())
    }

    async fn get_value<T>(&self, key: &str) -> Result<T, AggregationError>
    where
        T: FromRedisValue,
    {
        let redis = RedisConnectionManager::from_static();
        let mut conn = redis.get_mut_connection().await?;
        let result: T = conn.get(key).await?;
        Ok(result)
    }

    async fn get_hyperloglog_count(
        &self, key: &str,
    ) -> Result<u64, AggregationError> {
        let redis = RedisConnectionManager::from_static();
        let mut conn = redis.get_mut_connection().await?;
        let count: u64 = conn.pfcount(key).await?;
        Ok(count)
    }

    async fn get_hash_field(
        &self, key: &str, field: &str,
    ) -> Result<String, AggregationError> {
        let redis = RedisConnectionManager::from_static();
        let mut conn = redis.get_mut_connection().await?;
        let value: String = conn.hget(key, field).await?;
        Ok(value)
    }

    async fn set_expiry(
        &self, key: &str, seconds: i64,
    ) -> Result<(), AggregationError> {
        let redis = RedisConnectionManager::from_static();
        let mut conn = redis.get_mut_connection().await?;
        let _: bool = conn.expire(key, seconds).await?;
        Ok(())
    }
}

#[async_trait]
impl EventAggregator for RedisEventAggregator {
    #[instrument(skip(self, event))]
    async fn aggregate_event(
        &self, event: &EventAggregation,
    ) -> Result<(), AggregationError> {
        let buckets = [
            TimeBucket::Minute,
            TimeBucket::Hour,
            TimeBucket::Day,
            TimeBucket::Week,
            TimeBucket::Month,
        ];

        for bucket in buckets {
            let bucket_key =
                self.build_bucket_key(bucket, event.timestamp, None);

            let total_key = format!("{}:total", bucket_key);
            self.increment_counter(&total_key, 1).await?;

            let event_type_key =
                format!("{}:types:{}", bucket_key, event.event_type);
            self.increment_counter(&event_type_key, 1).await?;

            let users_hll_key = format!("{}:users_hll", bucket_key);
            self.add_to_hyperloglog(
                &users_hll_key,
                &event.user_id.to_string(),
            )
            .await?;

            if let Some(metadata) = &event.metadata {
                let metadata_key = format!("{}:metadata", bucket_key);
                let metadata_json = serde_json::to_string(metadata)?;
                self.update_hash(&metadata_key, "latest", &metadata_json)
                    .await?;
            }

            if let Some(expiry) = bucket.expiry_seconds() {
                self.set_expiry(&bucket_key, expiry).await?;
            }
        }

        debug!(
            "Aggregated event {} for user {} at {}",
            event.event_type, event.user_id, event.timestamp
        );

        Ok(())
    }

    #[instrument(skip(self))]
    async fn get_bucket_metrics(
        &self, bucket: TimeBucket, timestamp: DateTime<Utc>,
        filters: Option<AggregationFilters>,
    ) -> Result<BucketMetrics, AggregationError> {
        let bucket_key = self.build_bucket_key(bucket, timestamp, None);

        let total_key = format!("{}:total", bucket_key);
        let total_events: u64 = self.get_value(&total_key).await.unwrap_or(0);

        let users_hll_key = format!("{}:users_hll", bucket_key);
        let unique_users = self
            .get_hyperloglog_count(&users_hll_key)
            .await
            .unwrap_or(0);

        let mut event_type_counts = HashMap::new();

        if let Some(filters) = &filters {
            if let Some(event_types) = &filters.event_types {
                for event_type in event_types {
                    let type_key =
                        format!("{}:types:{}", bucket_key, event_type);
                    let count: u64 =
                        self.get_value(&type_key).await.unwrap_or(0);
                    if count > 0 {
                        let type_id =
                            event_type.chars().map(|c| c as u32).sum::<u32>()
                                as i32;
                        event_type_counts.insert(type_id, count);
                    }
                }
            }
        }

        let metadata_key = format!("{}:metadata", bucket_key);
        let properties = if let Ok(metadata_json) =
            self.get_hash_field(&metadata_key, "latest").await
        {
            serde_json::from_str(&metadata_json).unwrap_or_default()
        }
        else {
            HashMap::new()
        };

        let mut metrics = BucketMetrics::default();
        metrics.total_events = total_events;
        metrics.unique_users = unique_users;
        metrics.event_type_counts = event_type_counts;
        metrics.properties = properties;
        Ok(metrics)
    }

    #[instrument(skip(self))]
    async fn get_time_series(
        &self, bucket: TimeBucket, start: DateTime<Utc>, end: DateTime<Utc>,
        filters: Option<AggregationFilters>,
    ) -> Result<Vec<(String, BucketMetrics)>, AggregationError> {
        let mut result = Vec::new();
        let mut current = start;

        while current <= end {
            let bucket_key = bucket.bucket_key(current);
            let metrics = self
                .get_bucket_metrics(bucket, current, filters.clone())
                .await?;
            result.push((bucket_key, metrics));
            current += bucket.duration();
        }

        Ok(result)
    }
}
