use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct EventMetrics {
    pub total_events: i64,
    pub unique_users: i64,
    pub events_per_user: f64,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub top_events: Vec<EventTypeCount>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct UserMetrics {
    pub user_id: Uuid,
    pub total_events: i64,
    pub total_sessions: i64,
    pub total_time_spent: i64,     // in seconds
    pub avg_session_duration: f64, // in seconds
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub most_active_day: String,
    pub favorite_events: Vec<EventTypeCount>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct EventTypeCount {
    pub event_type: String,
    pub count: i64,
    pub percentage: f64,
}

/// Model for event_hourly_summaries materialized view
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct EventHourlySummary {
    pub event_type: String,
    pub hour: DateTime<Utc>,
    pub total_events: i64,
    pub unique_users: i64,
}

/// Model for user_daily_activity materialized view
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct UserDailyActivity {
    pub user_id: Uuid,
    pub date: DateTime<Utc>,
    pub total_events: i64,
    pub unique_event_types: i64,
    pub first_event: DateTime<Utc>,
    pub last_event: DateTime<Utc>,
}

/// Model for popular_events materialized view
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct PopularEvent {
    pub event_type: String,
    pub period: String,
    pub total_count: i64,
    pub unique_users: i64,
    pub growth_rate: Option<f64>,
}

/// Model for user_session_summaries materialized view
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct UserSessionSummary {
    pub user_id: Uuid,
    pub total_sessions: i64,
    pub avg_session_duration: f64, // in seconds
    pub total_time_spent: f64,     // in seconds
    pub avg_events_per_session: f64,
    pub first_session: DateTime<Utc>,
    pub last_session: DateTime<Utc>,
}

/// Model for page_analytics materialized view
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct PageAnalytics {
    pub page: Option<String>,
    pub hour: DateTime<Utc>,
    pub total_events: i64,
    pub unique_users: i64,
    pub unique_sessions: i64,
}

/// Model for product_analytics materialized view
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct ProductAnalytics {
    pub product_id: Option<i32>,
    pub event_type: String,
    pub date: DateTime<Utc>,
    pub total_events: i64,
    pub unique_users: i64,
}

/// Model for referrer_analytics materialized view
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct ReferrerAnalytics {
    pub referrer: Option<String>,
    pub date: DateTime<Utc>,
    pub total_events: i64,
    pub unique_users: i64,
    pub unique_sessions: i64,
}

/// Aggregated analytics response with all metrics
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct AnalyticsDashboard {
    pub overview: EventMetrics,
    pub popular_events: Vec<PopularEvent>,
    pub top_pages: Vec<PageAnalytics>,
    pub top_products: Vec<ProductAnalytics>,
    pub top_referrers: Vec<ReferrerAnalytics>,
    pub active_users: Vec<UserDailyActivity>,
}

/// Time-series data point for charts
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct TimeSeriesPoint {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
    pub label: Option<String>,
}

/// Chart data for frontend visualization
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct ChartData {
    pub title: String,
    pub chart_type: ChartType,
    pub data: Vec<TimeSeriesPoint>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub enum ChartType {
    Line,
    Bar,
    Pie,
    Area,
    Scatter,
}
