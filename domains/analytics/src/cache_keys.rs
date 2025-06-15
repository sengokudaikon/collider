use std::collections::HashMap;

use analytics_models::{
    EventHourlySummary, EventMetrics, PopularEvent, UserDailyActivity,
    UserMetrics,
};
use analytics_queries;
use redis_connection::cache_key;
use uuid::Uuid;

// Tiered cache keys for analytics domain

// User metrics caching - cached per user with time-based expiry
cache_key!(UserMetricsCacheKey::<UserMetrics> => "analytics:user_metrics:{}"[user_id: Uuid]);
cache_key!(UserMetricsTimeRangeCacheKey::<UserMetrics> => "analytics:user_metrics:{}:{}:{}"[user_id: Uuid, start: String, end: String]);

// Event metrics caching - cached per time range
cache_key!(EventMetricsCacheKey::<EventMetrics> => "analytics:event_metrics:{}:{}"[start: String, end: String]);
cache_key!(EventMetricsWithTypeCacheKey::<EventMetrics> => "analytics:event_metrics:{}:{}:{}"[start: String, end: String, event_type: String]);

// Real-time metrics buckets - hash maps containing event counts, user sets,
// and metadata
cache_key!(hash RealTimeMinuteBucketKey::<HashMap<String, i64>> => "analytics:metrics:minute:{}"[timestamp: String]);
cache_key!(hash RealTimeHourBucketKey::<HashMap<String, i64>> => "analytics:metrics:hour:{}"[timestamp: String]);
cache_key!(hash RealTimeDayBucketKey::<HashMap<String, i64>> => "analytics:metrics:day:{}"[timestamp: String]);

// Real-time unique user sets - for counting unique users per time bucket
cache_key!(set RealTimeMinuteUsersKey::<String> => "analytics:metrics:minute:{}:users"[timestamp: String]);
cache_key!(set RealTimeHourUsersKey::<String> => "analytics:metrics:hour:{}:users"[timestamp: String]);
cache_key!(set RealTimeDayUsersKey::<String> => "analytics:metrics:day:{}:users"[timestamp: String]);

// Popular events caching
cache_key!(PopularEventsCacheKey::<Vec<PopularEvent>> => "analytics:popular_events:{}"[period: String]);
cache_key!(PopularEventsWithLimitCacheKey::<Vec<PopularEvent>> => "analytics:popular_events:{}:{}"[period: String, limit: i64]);

// User daily activity caching
cache_key!(UserActivityCacheKey::<Vec<UserDailyActivity>> => "analytics:user_activity:{}:{}:{}"[user_id: Uuid, start: String, end: String]);
cache_key!(UserActivityAllCacheKey::<Vec<UserDailyActivity>> => "analytics:user_activity:all:{}:{}"[start: String, end: String]);

// Event hourly summaries caching
cache_key!(EventHourlySummariesCacheKey::<Vec<EventHourlySummary>> => "analytics:hourly_summaries:{}:{}"[start: String, end: String]);
cache_key!(EventHourlySummariesWithTypesCacheKey::<Vec<EventHourlySummary>> => "analytics:hourly_summaries:{}:{}:{}"[start: String, end: String, event_types: String]);

// Dashboard metrics cache - short lived cache for dashboard aggregations
cache_key!(DashboardMetricsCacheKey::<analytics_queries::DashboardMetrics> => "analytics:dashboard"[]);

// Time series analytics cache
cache_key!(TimeSeriesAnalyticsCacheKey::<Vec<EventHourlySummary>> => "analytics:time_series:{}:{}:{}"[bucket_type: String, start: String, end: String]);

// User lifecycle event aggregations - for tracking user registration, churn,
// etc.
cache_key!(hash UserLifecycleMetricsKey::<HashMap<String, i64>> => "analytics:user_lifecycle:{}"[date: String]);

// Session analytics - for tracking session patterns
cache_key!(SessionAnalyticsCacheKey::<HashMap<String, serde_json::Value>> => "analytics:sessions:{}:{}"[start: String, end: String]);
