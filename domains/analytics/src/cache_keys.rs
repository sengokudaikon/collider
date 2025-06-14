use std::collections::HashMap;

use analytics_models::{
    EventHourlySummary, EventMetrics, PopularEvent, UserDailyActivity,
    UserMetrics,
};
use analytics_queries;
use redis_connection::{core::value::Json, redis_key};
use uuid::Uuid;

// Tiered cache keys for analytics domain

// User metrics caching - cached per user with time-based expiry
redis_key!(UserMetricsCacheKey::<Json<UserMetrics>> => "analytics:user_metrics:{}"[user_id: Uuid]);
redis_key!(UserMetricsTimeRangeCacheKey::<Json<UserMetrics>> => "analytics:user_metrics:{}:{}:{}"[user_id: Uuid, start: String, end: String]);

// Event metrics caching - cached per time range
redis_key!(EventMetricsCacheKey::<Json<EventMetrics>> => "analytics:event_metrics:{}:{}"[start: String, end: String]);
redis_key!(EventMetricsWithTypeCacheKey::<Json<EventMetrics>> => "analytics:event_metrics:{}:{}:{}"[start: String, end: String, event_type: String]);

// Real-time metrics buckets - hash maps containing event counts, user sets,
// and metadata
redis_key!(hash RealTimeMinuteBucketKey::<HashMap<String, i64>> => "analytics:metrics:minute:{}"[timestamp: String]);
redis_key!(hash RealTimeHourBucketKey::<HashMap<String, i64>> => "analytics:metrics:hour:{}"[timestamp: String]);
redis_key!(hash RealTimeDayBucketKey::<HashMap<String, i64>> => "analytics:metrics:day:{}"[timestamp: String]);

// Real-time unique user sets - for counting unique users per time bucket
redis_key!(set RealTimeMinuteUsersKey::<String> => "analytics:metrics:minute:{}:users"[timestamp: String]);
redis_key!(set RealTimeHourUsersKey::<String> => "analytics:metrics:hour:{}:users"[timestamp: String]);
redis_key!(set RealTimeDayUsersKey::<String> => "analytics:metrics:day:{}:users"[timestamp: String]);

// Popular events caching
redis_key!(PopularEventsCacheKey::<Json<Vec<PopularEvent>>> => "analytics:popular_events:{}"[period: String]);
redis_key!(PopularEventsWithLimitCacheKey::<Json<Vec<PopularEvent>>> => "analytics:popular_events:{}:{}"[period: String, limit: i64]);

// User daily activity caching
redis_key!(UserActivityCacheKey::<Json<Vec<UserDailyActivity>>> => "analytics:user_activity:{}:{}:{}"[user_id: Uuid, start: String, end: String]);
redis_key!(UserActivityAllCacheKey::<Json<Vec<UserDailyActivity>>> => "analytics:user_activity:all:{}:{}"[start: String, end: String]);

// Event hourly summaries caching
redis_key!(EventHourlySummariesCacheKey::<Json<Vec<EventHourlySummary>>> => "analytics:hourly_summaries:{}:{}"[start: String, end: String]);
redis_key!(EventHourlySummariesWithTypesCacheKey::<Json<Vec<EventHourlySummary>>> => "analytics:hourly_summaries:{}:{}:{}"[start: String, end: String, event_types: String]);

// Dashboard metrics cache - short lived cache for dashboard aggregations
redis_key!(DashboardMetricsCacheKey::<Json<analytics_queries::DashboardMetrics>> => "analytics:dashboard"[]);

// Time series analytics cache
redis_key!(TimeSeriesAnalyticsCacheKey::<Json<Vec<EventHourlySummary>>> => "analytics:time_series:{}:{}:{}"[bucket_type: String, start: String, end: String]);

// User lifecycle event aggregations - for tracking user registration, churn,
// etc.
redis_key!(hash UserLifecycleMetricsKey::<HashMap<String, i64>> => "analytics:user_lifecycle:{}"[date: String]);

// Session analytics - for tracking session patterns
redis_key!(SessionAnalyticsCacheKey::<Json<HashMap<String, serde_json::Value>>> => "analytics:sessions:{}:{}"[start: String, end: String]);
