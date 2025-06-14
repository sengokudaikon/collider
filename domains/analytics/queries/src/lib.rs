use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

// ============================================================================
// Query Parameter Structs
// ============================================================================

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct HourlySummariesQuery {
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub event_types: Option<String>, // Comma-separated list
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct UserActivityQuery {
    pub user_id: Option<Uuid>,
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct PopularEventsQuery {
    pub period: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct RefreshViewsQuery {
    pub view_name: Option<String>,
    pub concurrent: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct EventMetricsQuery {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub event_type_filter: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct UserMetricsQuery {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct RealtimeMetricsQuery {
    pub timestamp: Option<DateTime<Utc>>,
}

// ============================================================================
// Response Structs
// ============================================================================

#[derive(Debug, Serialize, ToSchema)]
pub struct RefreshViewsResponse {
    pub refreshed_views: Vec<String>,
    pub duration_ms: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DashboardMetrics {
    pub total_events_today: i64,
    pub unique_users_today: i64,
    pub total_sessions_today: i64,
    pub avg_session_duration: f64,
    pub popular_events: Vec<analytics_models::EventTypeCount>,
    pub user_growth_this_week: i64,
    pub realtime_activity: HashMap<String, i64>,
}
