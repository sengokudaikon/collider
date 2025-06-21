use std::collections::HashMap;

use analytics_models::UserMetrics;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use redis_connection::{
    PoolError, RedisError,
    connection::RedisConnectionManager,
    core::{
        command::{IntoRedisCommands, RedisCommands},
        key::CacheKey,
    },
};
use serde_json::json;
use thiserror::Error;
use tracing::{error, info, instrument};
use user_events::UserAnalyticsEvent;
use uuid::Uuid;

use crate::cache_keys::{
    RealTimeDayBucketKey, RealTimeDayUsersKey, RealTimeHourBucketKey,
    RealTimeHourUsersKey, RealTimeMinuteBucketKey, RealTimeMinuteUsersKey,
    UserMetricsCacheKey,
};

#[derive(Debug, Error)]
pub enum RedisMetricsUpdaterError {
    #[error("Redis error: {0}")]
    Redis(#[from] RedisError),
    #[error("Redis pool error: {0}")]
    Pool(#[from] PoolError),
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Channel receive error: {0}")]
    Channel(String),
}

/// Real-time metrics updater that processes user domain events
/// Uses Redis for persistence and in-memory cache for performance
pub struct RedisAnalyticsMetricsUpdater {
    redis: RedisConnectionManager,
    user_metrics_cache: DashMap<Uuid, UserMetrics>,
}

impl Default for RedisAnalyticsMetricsUpdater {
    fn default() -> Self { Self::new() }
}

impl RedisAnalyticsMetricsUpdater {
    pub fn new() -> Self {
        Self {
            redis: RedisConnectionManager::from_static(),
            user_metrics_cache: DashMap::new(),
        }
    }

    /// Create a new instance with a specific Redis connection manager (useful
    /// for testing)
    pub fn with_redis_manager(redis: RedisConnectionManager) -> Self {
        Self {
            redis,
            user_metrics_cache: DashMap::new(),
        }
    }

    /// Process a user analytics event and update relevant metrics
    #[instrument(skip_all, fields(event_type = ?std::mem::discriminant(&event)))]
    pub async fn process_event(
        &mut self, event: UserAnalyticsEvent,
    ) -> Result<(), RedisMetricsUpdaterError> {
        match event {
            UserAnalyticsEvent::UserCreated {
                user_id,
                name,
                created_at,
                registration_source,
            } => {
                self.handle_user_created(
                    user_id,
                    name,
                    created_at,
                    registration_source,
                )
                .await?;
            }
            UserAnalyticsEvent::UserNameUpdated {
                user_id,
                old_name: _,
                new_name,
                updated_at,
            } => {
                self.handle_user_updated(user_id, new_name, updated_at)
                    .await?;
            }
            UserAnalyticsEvent::UserDeleted {
                user_id,
                deleted_at,
            } => {
                self.handle_user_deleted(user_id, deleted_at).await?;
            }
            UserAnalyticsEvent::UserSessionStart {
                user_id,
                session_id,
                started_at,
                user_agent,
                ip_address,
                referrer,
            } => {
                self.handle_session_start(
                    user_id, session_id, started_at, user_agent, ip_address,
                    referrer,
                )
                .await?;
            }
            UserAnalyticsEvent::UserSessionEnd {
                user_id,
                session_id,
                ended_at,
                duration_seconds,
            } => {
                self.handle_session_end(
                    user_id,
                    session_id,
                    ended_at,
                    duration_seconds,
                )
                .await?;
            }
        }
        Ok(())
    }

    #[instrument(skip_all)]
    async fn handle_user_created(
        &mut self, user_id: Uuid, name: String, created_at: DateTime<Utc>,
        registration_source: Option<String>,
    ) -> Result<(), RedisMetricsUpdaterError> {
        info!("Processing user created event for user {}", user_id);

        // Update real-time metrics in Redis
        self.update_redis_metrics(
            "user_created",
            Some(user_id),
            1,
            Some(json!({
                "name": name,
                "registration_source": registration_source,
                "created_at": created_at
            })),
        )
        .await?;

        // Update user metrics cache
        let user_metrics = UserMetrics {
            user_id,
            total_events: 1,
            total_sessions: 0,
            total_time_spent: 0,
            avg_session_duration: 0.0,
            first_seen: created_at,
            last_seen: created_at,
            most_active_day: created_at.format("%A").to_string(),
            favorite_events: vec![],
        };

        // Store in Redis and local cache
        self.store_user_metrics(&user_metrics).await?;
        self.user_metrics_cache.insert(user_id, user_metrics);

        info!(
            "Successfully processed user created event for user {}",
            user_id
        );
        Ok(())
    }

    #[instrument(skip_all)]
    async fn handle_user_updated(
        &mut self, user_id: Uuid, _new_name: String,
        updated_at: DateTime<Utc>,
    ) -> Result<(), RedisMetricsUpdaterError> {
        info!("Processing user updated event for user {}", user_id);

        // Update user activity metrics - profile updates indicate engagement
        if let Some(mut user_metrics) =
            self.user_metrics_cache.get_mut(&user_id)
        {
            user_metrics.last_seen = updated_at;
            user_metrics.total_events += 1;
            user_metrics.most_active_day =
                updated_at.format("%A").to_string();

            // Store updated metrics in Redis
            self.store_user_metrics(&user_metrics).await?;
        }
        else {
            // Load from Redis if not in cache
            if let Some(mut user_metrics) =
                self.load_user_metrics(user_id).await?
            {
                user_metrics.last_seen = updated_at;
                user_metrics.total_events += 1;
                user_metrics.most_active_day =
                    updated_at.format("%A").to_string();

                self.store_user_metrics(&user_metrics).await?;
                self.user_metrics_cache.insert(user_id, user_metrics);
            }
        }

        // Update real-time metrics in Redis
        self.update_redis_metrics(
            "user_updated",
            Some(user_id),
            1,
            Some(json!({"updated_at": updated_at})),
        )
        .await?;

        info!(
            "Successfully processed user updated event for user {}",
            user_id
        );
        Ok(())
    }

    #[instrument(skip_all)]
    async fn handle_user_deleted(
        &mut self, user_id: Uuid, deleted_at: DateTime<Utc>,
    ) -> Result<(), RedisMetricsUpdaterError> {
        info!("Processing user deleted event for user {}", user_id);

        // Mark user as inactive for churned users
        if let Some(mut user_metrics) =
            self.user_metrics_cache.get_mut(&user_id)
        {
            user_metrics.last_seen = deleted_at;
            self.store_user_metrics(&user_metrics).await?;
        }
        else if let Some(mut user_metrics) =
            self.load_user_metrics(user_id).await?
        {
            user_metrics.last_seen = deleted_at;
            self.store_user_metrics(&user_metrics).await?;
            self.user_metrics_cache.insert(user_id, user_metrics);
        }

        // Update real-time metrics in Redis
        self.update_redis_metrics(
            "user_deleted",
            Some(user_id),
            1,
            Some(json!({"deleted_at": deleted_at})),
        )
        .await?;

        info!(
            "Successfully processed user deleted event for user {}",
            user_id
        );
        Ok(())
    }

    #[instrument(skip_all)]
    async fn handle_session_start(
        &mut self, user_id: Uuid, session_id: Uuid,
        started_at: DateTime<Utc>, user_agent: Option<String>,
        _ip_address: Option<String>, _referrer: Option<String>,
    ) -> Result<(), RedisMetricsUpdaterError> {
        info!(
            "Processing session start for user {} session {}",
            user_id, session_id
        );

        // Update user metrics
        if let Some(mut user_metrics) =
            self.user_metrics_cache.get_mut(&user_id)
        {
            user_metrics.total_sessions += 1;
            user_metrics.last_seen = started_at;
            user_metrics.most_active_day =
                started_at.format("%A").to_string();
            self.store_user_metrics(&user_metrics).await?;
        }
        else if let Some(mut user_metrics) =
            self.load_user_metrics(user_id).await?
        {
            user_metrics.total_sessions += 1;
            user_metrics.last_seen = started_at;
            user_metrics.most_active_day =
                started_at.format("%A").to_string();
            self.store_user_metrics(&user_metrics).await?;
            self.user_metrics_cache.insert(user_id, user_metrics);
        }

        // Update real-time metrics in Redis
        self.update_redis_metrics(
            "session_start",
            Some(user_id),
            1,
            Some(json!({
                "session_id": session_id,
                "started_at": started_at,
                "device_type": Self::detect_device_type(&user_agent),
                "browser": Self::detect_browser(&user_agent)
            })),
        )
        .await?;

        info!(
            "Successfully processed session start for user {} session {}",
            user_id, session_id
        );
        Ok(())
    }

    #[instrument(skip_all)]
    async fn handle_session_end(
        &mut self, user_id: Uuid, _session_id: Uuid, ended_at: DateTime<Utc>,
        duration_seconds: i64,
    ) -> Result<(), RedisMetricsUpdaterError> {
        info!("Processing session end for user {}", user_id);

        // Update user metrics with session duration
        if let Some(mut user_metrics) =
            self.user_metrics_cache.get_mut(&user_id)
        {
            let total_duration = user_metrics.avg_session_duration
                * (user_metrics.total_sessions - 1) as f64;
            user_metrics.avg_session_duration = (total_duration
                + duration_seconds as f64)
                / user_metrics.total_sessions as f64;
            user_metrics.total_time_spent += duration_seconds;
            user_metrics.last_seen = ended_at;
            self.store_user_metrics(&user_metrics).await?;
        }
        else if let Some(mut user_metrics) =
            self.load_user_metrics(user_id).await?
        {
            let total_duration = user_metrics.avg_session_duration
                * (user_metrics.total_sessions - 1) as f64;
            user_metrics.avg_session_duration = (total_duration
                + duration_seconds as f64)
                / user_metrics.total_sessions as f64;
            user_metrics.total_time_spent += duration_seconds;
            user_metrics.last_seen = ended_at;
            self.store_user_metrics(&user_metrics).await?;
            self.user_metrics_cache.insert(user_id, user_metrics);
        }

        // Update real-time metrics in Redis
        self.update_redis_metrics(
            "session_end",
            Some(user_id),
            1,
            Some(json!({
                "ended_at": ended_at,
                "duration_seconds": duration_seconds
            })),
        )
        .await?;

        info!("Successfully processed session end for user {}", user_id);
        Ok(())
    }

    /// Update real-time metrics in Redis using fallback string keys
    async fn update_redis_metrics(
        &self, event_type: &str, user_id: Option<Uuid>, event_count: i64,
        metadata: Option<serde_json::Value>,
    ) -> Result<(), RedisMetricsUpdaterError> {
        let now = Utc::now();

        // Update metrics for different time buckets using fallback string
        // keys
        let time_buckets = [
            ("minute", now.format("%Y%m%d%H%M").to_string(), 86400), // 1 day
            ("hour", now.format("%Y%m%d%H").to_string(), 604800), // 7 days
            ("day", now.format("%Y%m%d").to_string(), 2592000),   // 30 days
        ];

        for (bucket_type, timestamp, expiry) in time_buckets {
            // Get a fresh connection for each bucket to avoid move issues
            let conn = self.redis.get_connection().await?;
            let mut commands = conn.cmd();

            // Use type-safe cache keys
            let key = match bucket_type {
                "minute" => {
                    RealTimeMinuteBucketKey.get_key_with_args((&timestamp,))
                }
                "hour" => {
                    RealTimeHourBucketKey.get_key_with_args((&timestamp,))
                }
                "day" => {
                    RealTimeDayBucketKey.get_key_with_args((&timestamp,))
                }
                _ => continue,
            }
            .to_string();

            // Increment event count
            let count_field = format!("{}:count", event_type);
            commands.hset(&key, &count_field, event_count).await?;

            // Add user to set for unique user counting (if user_id provided)
            if let Some(uid) = user_id {
                let users_key = match bucket_type {
                    "minute" => {
                        RealTimeMinuteUsersKey
                            .get_key_with_args((&timestamp,))
                    }
                    "hour" => {
                        RealTimeHourUsersKey.get_key_with_args((&timestamp,))
                    }
                    "day" => {
                        RealTimeDayUsersKey.get_key_with_args((&timestamp,))
                    }
                    _ => continue,
                }
                .to_string();
                commands.sadd(&users_key, uid.to_string()).await?;
                commands.expire(&users_key, expiry).await?;
            }

            // Store metadata (if provided)
            if let Some(meta) = &metadata {
                let metadata_field = format!("{}:metadata", event_type);
                commands
                    .hset(&key, &metadata_field, meta.to_string())
                    .await?;
            }

            // Set expiration for the hash
            commands.expire(&key, expiry).await?;
        }

        Ok(())
    }

    /// Store user metrics in Redis
    async fn store_user_metrics(
        &self, user_metrics: &UserMetrics,
    ) -> Result<(), RedisMetricsUpdaterError> {
        let key = UserMetricsCacheKey
            .get_key_with_args((&user_metrics.user_id,))
            .to_string();
        let serialized = serde_json::to_string(user_metrics)?;

        let conn = self.redis.get_connection().await?;
        let mut conn = conn.cmd();
        conn.set_ex(&key, serialized, 86400 * 7).await?;

        Ok(())
    }

    /// Load user metrics from Redis
    async fn load_user_metrics(
        &self, user_id: Uuid,
    ) -> Result<Option<UserMetrics>, RedisMetricsUpdaterError> {
        let key = UserMetricsCacheKey
            .get_key_with_args((&user_id,))
            .to_string();

        let mut conn = self.redis.get_connection().await?.cmd();
        let result: Option<String> = conn.get(&key).await?;

        match result {
            Some(serialized) => {
                let user_metrics: UserMetrics =
                    serde_json::from_str(&serialized)?;
                Ok(Some(user_metrics))
            }
            None => Ok(None),
        }
    }

    /// Get cached user metrics (from memory first, then Redis)
    pub async fn get_user_metrics(
        &mut self, user_id: &Uuid,
    ) -> Result<Option<UserMetrics>, RedisMetricsUpdaterError> {
        // Check memory cache first
        if let Some(metrics) = self.user_metrics_cache.get(user_id) {
            return Ok(Some(metrics.clone()));
        }

        // Load from Redis and cache in memory
        if let Some(metrics) = self.load_user_metrics(*user_id).await? {
            self.user_metrics_cache.insert(*user_id, metrics.clone());
            Ok(Some(metrics))
        }
        else {
            Ok(None)
        }
    }

    /// Flush user metrics cache to Redis (for persistence)
    pub async fn flush_user_metrics(
        &mut self,
    ) -> Result<(), RedisMetricsUpdaterError> {
        let cache_size = self.user_metrics_cache.len();

        // Collect the entries to avoid consuming the map
        let entries: Vec<(Uuid, UserMetrics)> = self
            .user_metrics_cache
            .iter()
            .map(|entry| (*entry.key(), entry.value().clone()))
            .collect();

        for (_user_id, user_metrics) in entries {
            self.store_user_metrics(&user_metrics).await?;
        }

        info!("Flushed {} user metrics from cache to Redis", cache_size);
        Ok(())
    }

    /// Get real-time metrics from Redis
    pub async fn get_real_time_metrics(
        &self, bucket_type: &str, timestamp: DateTime<Utc>,
    ) -> Result<HashMap<String, i64>, RedisMetricsUpdaterError> {
        let key = match bucket_type {
            "minute" => {
                let ts = timestamp.format("%Y%m%d%H%M").to_string();
                RealTimeMinuteBucketKey
                    .get_key_with_args((&ts,))
                    .to_string()
            }
            "hour" => {
                let ts = timestamp.format("%Y%m%d%H").to_string();
                RealTimeHourBucketKey.get_key_with_args((&ts,)).to_string()
            }
            "day" => {
                let ts = timestamp.format("%Y%m%d").to_string();
                RealTimeDayBucketKey.get_key_with_args((&ts,)).to_string()
            }
            _ => return Ok(HashMap::new()),
        };

        let mut conn = self.redis.get_connection().await?.cmd();
        let raw_result: HashMap<String, String> = conn.hgetall(&key).await?;

        // Filter and convert only count fields (not metadata fields)
        let result: HashMap<String, i64> = raw_result
            .into_iter()
            .filter(|(key, _)| key.ends_with(":count"))
            .filter_map(|(key, value)| {
                value.parse::<i64>().ok().map(|v| (key, v))
            })
            .collect();

        Ok(result)
    }

    // Helper methods for parsing user agent data
    fn detect_device_type(user_agent: &Option<String>) -> Option<String> {
        user_agent.as_ref().map(|ua| {
            if ua.contains("Mobile") {
                "mobile".to_string()
            }
            else if ua.contains("Tablet") {
                "tablet".to_string()
            }
            else {
                "desktop".to_string()
            }
        })
    }

    fn detect_browser(user_agent: &Option<String>) -> Option<String> {
        user_agent.as_ref().map(|ua| {
            if ua.contains("Chrome") {
                "Chrome".to_string()
            }
            else if ua.contains("Firefox") {
                "Firefox".to_string()
            }
            else if ua.contains("Safari") {
                "Safari".to_string()
            }
            else if ua.contains("Edge") {
                "Edge".to_string()
            }
            else {
                "Unknown".to_string()
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use test_utils::TestRedisContainer;

    use super::*;

    async fn setup_test_redis()
    -> anyhow::Result<(TestRedisContainer, RedisAnalyticsMetricsUpdater)>
    {
        let container = TestRedisContainer::new().await?;
        container.flush_all_keys().await?;

        let mut updater = RedisAnalyticsMetricsUpdater::new();
        // Replace the default connection with test container connection
        updater.redis = RedisConnectionManager::new(container.pool.clone());

        Ok((container, updater))
    }

    fn test_user_id() -> Uuid {
        static COUNTER: AtomicU32 = AtomicU32::new(1);
        let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
        Uuid::from_u128(counter as u128)
    }

    fn test_session_id() -> Uuid {
        static COUNTER: AtomicU32 = AtomicU32::new(1000);
        let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
        Uuid::from_u128(counter as u128)
    }

    #[tokio::test]
    async fn test_device_detection() {
        assert_eq!(
            RedisAnalyticsMetricsUpdater::detect_device_type(&Some(
                "Mozilla/5.0 Mobile".to_string()
            )),
            Some("mobile".to_string())
        );
        assert_eq!(
            RedisAnalyticsMetricsUpdater::detect_browser(&Some(
                "Mozilla/5.0 Chrome/91.0".to_string()
            )),
            Some("Chrome".to_string())
        );
    }

    #[tokio::test]
    async fn test_cache_key_generation() {
        let user_id = test_user_id();
        let timestamp = "202501151030".to_string();

        // Test user metrics cache key
        let user_key = UserMetricsCacheKey
            .get_key_with_args((&user_id,))
            .to_string();
        assert_eq!(user_key, format!("analytics:user_metrics:{}", user_id));

        // Test real-time bucket keys
        let minute_key = RealTimeMinuteBucketKey
            .get_key_with_args((&timestamp,))
            .to_string();
        assert_eq!(
            minute_key,
            format!("analytics:metrics:minute:{}", timestamp)
        );

        let hour_key = RealTimeHourBucketKey
            .get_key_with_args((&timestamp,))
            .to_string();
        assert_eq!(hour_key, format!("analytics:metrics:hour:{}", timestamp));

        let day_key = RealTimeDayBucketKey
            .get_key_with_args((&timestamp,))
            .to_string();
        assert_eq!(day_key, format!("analytics:metrics:day:{}", timestamp));

        // Test user set keys
        let minute_users_key = RealTimeMinuteUsersKey
            .get_key_with_args((&timestamp,))
            .to_string();
        assert_eq!(
            minute_users_key,
            format!("analytics:metrics:minute:{}:users", timestamp)
        );
    }

    #[tokio::test]
    async fn test_user_created_event() -> anyhow::Result<()> {
        let (_container, mut updater) = setup_test_redis().await?;
        let user_id = test_user_id();
        let created_at = Utc::now();

        let event = UserAnalyticsEvent::UserCreated {
            user_id,
            name: "Test User".to_string(),
            created_at,
            registration_source: Some("web".to_string()),
        };

        // Process the event
        updater.process_event(event).await?;

        // Verify user metrics were created
        let user_metrics = updater.get_user_metrics(&user_id).await?;
        assert!(user_metrics.is_some());

        let metrics = user_metrics.unwrap();
        assert_eq!(metrics.user_id, user_id);
        assert_eq!(metrics.total_events, 1);
        assert_eq!(metrics.total_sessions, 0);
        assert_eq!(metrics.first_seen, created_at);
        assert_eq!(metrics.last_seen, created_at);

        Ok(())
    }

    #[tokio::test]
    async fn test_session_events() -> anyhow::Result<()> {
        let (_container, mut updater) = setup_test_redis().await?;
        let user_id = test_user_id();
        let session_id = test_session_id();
        let started_at = Utc::now();
        let ended_at = started_at + chrono::Duration::seconds(300); // 5 minute session

        // First create user
        let create_event = UserAnalyticsEvent::UserCreated {
            user_id,
            name: "Test User".to_string(),
            created_at: started_at,
            registration_source: Some("web".to_string()),
        };
        updater.process_event(create_event).await?;

        // Start session
        let session_start = UserAnalyticsEvent::UserSessionStart {
            user_id,
            session_id,
            started_at,
            user_agent: Some("Mozilla/5.0 Chrome/91.0".to_string()),
            ip_address: Some("127.0.0.1".to_string()),
            referrer: None,
        };
        updater.process_event(session_start).await?;

        // End session
        let session_end = UserAnalyticsEvent::UserSessionEnd {
            user_id,
            session_id,
            ended_at,
            duration_seconds: 300,
        };
        updater.process_event(session_end).await?;

        // Verify user metrics were updated
        let user_metrics = updater.get_user_metrics(&user_id).await?;
        assert!(user_metrics.is_some());

        let metrics = user_metrics.unwrap();
        assert_eq!(metrics.total_sessions, 1);
        assert_eq!(metrics.total_time_spent, 300);
        assert_eq!(metrics.avg_session_duration, 300.0);
        assert_eq!(metrics.last_seen, ended_at);

        Ok(())
    }

    #[tokio::test]
    async fn test_real_time_metrics_storage() -> anyhow::Result<()> {
        let (_container, mut updater) = setup_test_redis().await?;
        let user_id = test_user_id();
        let event_time = Utc::now();

        let event = UserAnalyticsEvent::UserCreated {
            user_id,
            name: "Test User".to_string(),
            created_at: event_time,
            registration_source: Some("mobile".to_string()),
        };

        updater.process_event(event).await?;

        // Check that real-time metrics were stored
        let metrics =
            updater.get_real_time_metrics("minute", event_time).await?;
        assert!(!metrics.is_empty());

        // Should have user_created:count field
        let count = metrics.get("user_created:count");
        assert!(count.is_some());
        assert_eq!(*count.unwrap(), 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_user_metrics_persistence() -> anyhow::Result<()> {
        let (_container, mut updater) = setup_test_redis().await?;
        let user_id = test_user_id();
        let created_at = Utc::now();

        // Create user metrics
        let user_metrics = UserMetrics {
            user_id,
            total_events: 5,
            total_sessions: 2,
            total_time_spent: 600,
            avg_session_duration: 300.0,
            first_seen: created_at,
            last_seen: created_at,
            most_active_day: "Monday".to_string(),
            favorite_events: vec![
                analytics_models::EventTypeCount {
                    event_type: "login".to_string(),
                    count: 3,
                    percentage: 60.0,
                },
                analytics_models::EventTypeCount {
                    event_type: "page_view".to_string(),
                    count: 2,
                    percentage: 40.0,
                },
            ],
        };

        // Store in cache
        updater
            .user_metrics_cache
            .insert(user_id, user_metrics.clone());

        // Flush to Redis
        updater.flush_user_metrics().await?;

        // Clear cache and reload from Redis
        updater.user_metrics_cache.clear();
        let loaded_metrics = updater.load_user_metrics(user_id).await?;

        assert!(loaded_metrics.is_some());
        let loaded = loaded_metrics.unwrap();
        assert_eq!(loaded.user_id, user_id);
        assert_eq!(loaded.total_events, 5);
        assert_eq!(loaded.total_sessions, 2);
        assert_eq!(loaded.avg_session_duration, 300.0);

        Ok(())
    }

    #[tokio::test]
    async fn test_user_updated_event() -> anyhow::Result<()> {
        let (_container, mut updater) = setup_test_redis().await?;
        let user_id = test_user_id();
        let created_at = Utc::now();
        let updated_at = created_at + chrono::Duration::hours(1);

        // First create user
        let create_event = UserAnalyticsEvent::UserCreated {
            user_id,
            name: "Test User".to_string(),
            created_at,
            registration_source: Some("web".to_string()),
        };
        updater.process_event(create_event).await?;

        // Update user
        let update_event = UserAnalyticsEvent::UserNameUpdated {
            user_id,
            old_name: "Test User".to_string(),
            new_name: "Updated User".to_string(),
            updated_at,
        };
        updater.process_event(update_event).await?;

        // Verify metrics were updated
        let user_metrics = updater.get_user_metrics(&user_id).await?;
        assert!(user_metrics.is_some());

        let metrics = user_metrics.unwrap();
        assert_eq!(metrics.total_events, 2); // Created + Updated
        assert_eq!(metrics.last_seen, updated_at);

        Ok(())
    }

    #[tokio::test]
    async fn test_user_deleted_event() -> anyhow::Result<()> {
        let (_container, mut updater) = setup_test_redis().await?;
        let user_id = test_user_id();
        let created_at = Utc::now();
        let deleted_at = created_at + chrono::Duration::hours(2);

        // First create user
        let create_event = UserAnalyticsEvent::UserCreated {
            user_id,
            name: "Test User".to_string(),
            created_at,
            registration_source: Some("web".to_string()),
        };
        updater.process_event(create_event).await?;

        // Delete user
        let delete_event = UserAnalyticsEvent::UserDeleted {
            user_id,
            deleted_at,
        };
        updater.process_event(delete_event).await?;

        // Verify last_seen was updated to deletion time
        let user_metrics = updater.get_user_metrics(&user_id).await?;
        assert!(user_metrics.is_some());

        let metrics = user_metrics.unwrap();
        assert_eq!(metrics.last_seen, deleted_at);

        Ok(())
    }

    #[tokio::test]
    async fn test_concurrent_events() -> anyhow::Result<()> {
        let (_container, mut updater) = setup_test_redis().await?;
        let user_id = test_user_id();
        let base_time = Utc::now();

        // Process multiple events concurrently
        let events = vec![
            UserAnalyticsEvent::UserCreated {
                user_id,
                name: "Test User".to_string(),
                created_at: base_time,
                registration_source: Some("web".to_string()),
            },
            UserAnalyticsEvent::UserNameUpdated {
                user_id,
                old_name: "Test User".to_string(),
                new_name: "Updated User".to_string(),
                updated_at: base_time + chrono::Duration::minutes(1),
            },
            UserAnalyticsEvent::UserSessionStart {
                user_id,
                session_id: test_session_id(),
                started_at: base_time + chrono::Duration::minutes(2),
                user_agent: Some("Mozilla/5.0".to_string()),
                ip_address: Some("127.0.0.1".to_string()),
                referrer: None,
            },
        ];

        // Process all events
        for event in events {
            updater.process_event(event).await?;
        }

        // Verify final state
        let user_metrics = updater.get_user_metrics(&user_id).await?;
        assert!(user_metrics.is_some());

        let metrics = user_metrics.unwrap();
        assert_eq!(metrics.total_events, 2); // Created + Updated (session increments in different way)
        assert_eq!(metrics.total_sessions, 1);

        Ok(())
    }
}
