use std::collections::HashMap;

use analytics_models::UserMetrics;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use redis_connection::{
    connection::RedisConnectionManager, core::command::{IntoRedisCommands, RedisCommands},
    PoolError,
    RedisError,
};
use serde_json::json;
use thiserror::Error;
use tracing::{error, info, instrument};
use user_events::UserAnalyticsEvent;
use uuid::Uuid;

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

            // Use fallback string keys since type-safe keys seem to have
            // issues
            let key =
                format!("analytics:metrics:{}:{}", bucket_type, timestamp);

            // Increment event count
            let count_field = format!("{}:count", event_type);
            commands.hset(&key, &count_field, event_count).await?;

            // Add user to set for unique user counting (if user_id provided)
            if let Some(uid) = user_id {
                let users_key = format!("{}:users", key);
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
        let key = format!("analytics:user_metrics:{}", user_metrics.user_id);
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
        let key = format!("analytics:user_metrics:{}", user_id);

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
                format!(
                    "analytics:metrics:minute:{}",
                    timestamp.format("%Y%m%d%H%M")
                )
            }
            "hour" => {
                format!(
                    "analytics:metrics:hour:{}",
                    timestamp.format("%Y%m%d%H")
                )
            }
            "day" => {
                format!(
                    "analytics:metrics:day:{}",
                    timestamp.format("%Y%m%d")
                )
            }
            _ => return Ok(HashMap::new()),
        };

        let mut conn = self.redis.get_connection().await?.cmd();
        let result: HashMap<String, i64> = conn.hgetall(&key).await?;

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
    use super::*;

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
}
