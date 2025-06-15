use std::{collections::HashMap, sync::Arc};

use analytics::RedisAnalyticsMetricsUpdater;
use analytics_dao::AnalyticsViewsDao;
use analytics_models::{
    EventHourlySummary, EventMetrics, PopularEvent, UserDailyActivity,
    UserMetrics,
};
use analytics_queries::{
    DashboardMetrics, EventMetricsQuery, HourlySummariesQuery,
    PopularEventsQuery, RealtimeMetricsQuery, RefreshViewsQuery,
    RefreshViewsResponse, UserActivityQuery, UserMetricsQuery,
};
use axum::{
    Router,
    extract::{Path, Query, State},
    response::Json,
    routing::{get, post},
};
use chrono::Utc;
use domain::{AppError, AppResult};
use sql_connection::SqlConnect;
use tracing::instrument;
use uuid::Uuid;

#[derive(Clone)]
pub struct AnalyticsServices {
    pub dao: Arc<AnalyticsViewsDao>,
    pub redis_updater: Arc<tokio::sync::Mutex<RedisAnalyticsMetricsUpdater>>,
}

impl AnalyticsServices {
    pub fn new(
        db: SqlConnect, redis_updater: RedisAnalyticsMetricsUpdater,
    ) -> Self {
        Self {
            dao: Arc::new(AnalyticsViewsDao::new(db)),
            redis_updater: Arc::new(tokio::sync::Mutex::new(redis_updater)),
        }
    }
}

pub struct AnalyticsHandlers;

impl AnalyticsHandlers {
    pub fn routes() -> Router<AnalyticsServices> {
        Router::new()
            // View endpoints
            .route("/views/hourly-summaries", get(get_hourly_summaries))
            .route("/views/user-activity", get(get_user_activity))
            .route("/views/popular-events", get(get_popular_events))
            .route("/views/refresh", post(refresh_views))
            // Metrics endpoints
            .route("/metrics/events", get(get_event_metrics))
            .route("/metrics/users/:user_id", get(get_user_metrics))
            .route(
                "/metrics/realtime/:bucket_type",
                get(get_realtime_metrics),
            )
            .route("/metrics/dashboard", get(get_dashboard_metrics))
    }
}

// ============================================================================
// View Handlers
// ============================================================================

#[utoipa::path(
    get,
    path = "/api/analytics/views/hourly-summaries",
    params(HourlySummariesQuery),
    responses(
        (status = 200, description = "Event hourly summaries", body = Vec<EventHourlySummary>),
        (status = 400, description = "Invalid query parameters"),
        (status = 500, description = "Internal server error")
    ),
    tag = "analytics-views"
)]
#[instrument(skip_all)]
pub async fn get_hourly_summaries(
    State(services): State<AnalyticsServices>,
    Query(params): Query<HourlySummariesQuery>,
) -> Result<Json<Vec<EventHourlySummary>>, AppError> {
    let event_types = params.event_types.map(|types| {
        types.split(',').map(|s| s.trim().to_string()).collect()
    });

    let summaries = services
        .dao
        .get_event_hourly_summaries(
            params.start_time,
            params.end_time,
            event_types,
            params.limit,
        )
        .await?;

    Ok(Json(summaries))
}

#[utoipa::path(
    get,
    path = "/api/analytics/views/user-activity",
    params(UserActivityQuery),
    responses(
        (status = 200, description = "User daily activity", body = Vec<UserDailyActivity>),
        (status = 400, description = "Invalid query parameters"),
        (status = 500, description = "Internal server error")
    ),
    tag = "analytics-views"
)]
#[instrument(skip_all)]
pub async fn get_user_activity(
    State(services): State<AnalyticsServices>,
    Query(params): Query<UserActivityQuery>,
) -> Result<Json<Vec<UserDailyActivity>>, AppError> {
    let activities = services
        .dao
        .get_user_daily_activity(
            params.user_id,
            params.start_date,
            params.end_date,
            params.limit,
        )
        .await?;

    Ok(Json(activities))
}

#[utoipa::path(
    get,
    path = "/api/analytics/views/popular-events",
    params(PopularEventsQuery),
    responses(
        (status = 200, description = "Popular events", body = Vec<PopularEvent>),
        (status = 400, description = "Invalid query parameters"),
        (status = 500, description = "Internal server error")
    ),
    tag = "analytics-views"
)]
#[instrument(skip_all)]
pub async fn get_popular_events(
    State(services): State<AnalyticsServices>,
    Query(params): Query<PopularEventsQuery>,
) -> Result<Json<Vec<PopularEvent>>, AppError> {
    let events = services
        .dao
        .get_popular_events(params.period, params.limit)
        .await?;

    Ok(Json(events))
}

#[utoipa::path(
    post,
    path = "/api/analytics/views/refresh",
    params(RefreshViewsQuery),
    responses(
        (status = 200, description = "Views refreshed successfully", body = RefreshViewsResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "analytics-views"
)]
#[instrument(skip_all)]
pub async fn refresh_views(
    State(services): State<AnalyticsServices>,
    Query(params): Query<RefreshViewsQuery>,
) -> Result<Json<RefreshViewsResponse>, AppError> {
    let command = analytics_commands::RefreshViewsCommand {
        view_name: params.view_name,
        concurrent: params.concurrent.unwrap_or(true),
    };

    let response = services.dao.refresh_views(command).await?;

    Ok(Json(RefreshViewsResponse {
        refreshed_views: response.refreshed_views,
        duration_ms: response.duration_ms,
    }))
}

// ============================================================================
// Metrics Handlers
// ============================================================================

#[utoipa::path(
    get,
    path = "/api/analytics/metrics/events",
    params(EventMetricsQuery),
    responses(
        (status = 200, description = "Event metrics", body = EventMetrics),
        (status = 400, description = "Invalid query parameters"),
        (status = 500, description = "Internal server error")
    ),
    tag = "analytics-metrics"
)]
#[instrument(skip_all)]
pub async fn get_event_metrics(
    State(services): State<AnalyticsServices>,
    Query(params): Query<EventMetricsQuery>,
) -> AppResult<Json<EventMetrics>> {
    let metrics = services
        .dao
        .get_event_metrics(params.start, params.end, params.event_type_filter)
        .await?;

    Ok(Json(metrics))
}

#[utoipa::path(
    get,
    path = "/api/analytics/metrics/users/{user_id}",
    params(
        ("user_id" = Uuid, Path, description = "User ID"),
        UserMetricsQuery
    ),
    responses(
        (status = 200, description = "User metrics", body = UserMetrics),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "analytics-metrics"
)]
#[instrument(skip_all)]
pub async fn get_user_metrics(
    State(services): State<AnalyticsServices>, Path(user_id): Path<Uuid>,
    Query(params): Query<UserMetricsQuery>,
) -> AppResult<Json<UserMetrics>> {
    // First try to get cached metrics from Redis
    let mut redis_updater = services.redis_updater.lock().await;
    if let Ok(Some(cached_metrics)) =
        redis_updater.get_user_metrics(&user_id).await
    {
        return Ok(Json(cached_metrics));
    }
    drop(redis_updater);

    // Fall back to database query
    let metrics = services
        .dao
        .get_user_metrics(user_id, params.start, params.end)
        .await?;

    Ok(Json(metrics))
}

#[utoipa::path(
    get,
    path = "/api/analytics/metrics/realtime/{bucket_type}",
    params(
        ("bucket_type" = String, Path, description = "Time bucket type (minute, hour, day)"),
        RealtimeMetricsQuery
    ),
    responses(
        (status = 200, description = "Realtime metrics", body = HashMap<String, i64>),
        (status = 400, description = "Invalid bucket type"),
        (status = 500, description = "Internal server error")
    ),
    tag = "analytics-metrics"
)]
#[instrument(skip_all)]
pub async fn get_realtime_metrics(
    State(services): State<AnalyticsServices>,
    Path(bucket_type): Path<String>,
    Query(params): Query<RealtimeMetricsQuery>,
) -> AppResult<Json<HashMap<String, i64>>> {
    let timestamp = params.timestamp.unwrap_or_else(Utc::now);

    if !["minute", "hour", "day"].contains(&bucket_type.as_str()) {
        return Err(anyhow::anyhow!(
            "Invalid bucket type. Must be 'minute', 'hour', or 'day'"
        )
        .into());
    }

    let redis_updater = services.redis_updater.lock().await;
    let metrics = redis_updater
        .get_real_time_metrics(&bucket_type, timestamp)
        .await?;

    Ok(Json(metrics))
}

#[utoipa::path(
    get,
    path = "/api/analytics/metrics/dashboard",
    responses(
        (status = 200, description = "Dashboard metrics overview", body = DashboardMetrics),
        (status = 500, description = "Internal server error")
    ),
    tag = "analytics-metrics"
)]
#[instrument(skip_all)]
pub async fn get_dashboard_metrics(
    State(services): State<AnalyticsServices>,
) -> AppResult<Json<DashboardMetrics>> {
    let now = Utc::now();
    let today_start =
        now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
    let week_start = now - chrono::Duration::days(7);

    // Get today's event metrics
    let event_metrics = services
        .dao
        .get_event_metrics(today_start, now, None)
        .await?;

    // Get realtime activity from Redis
    let redis_updater = services.redis_updater.lock().await;
    let realtime_activity =
        redis_updater.get_real_time_metrics("hour", now).await?;

    // Get weekly event metrics for growth calculation
    let week_metrics = services
        .dao
        .get_event_metrics(week_start, now, None)
        .await?;

    let dashboard = DashboardMetrics {
        total_events_today: event_metrics.total_events,
        unique_users_today: event_metrics.unique_users,
        total_sessions_today: *realtime_activity
            .get("session_start:count")
            .unwrap_or(&0),
        avg_session_duration: (*realtime_activity
            .get("session_end:metadata")
            .unwrap_or(&0)) as f64,
        popular_events: event_metrics.top_events,
        user_growth_this_week: week_metrics.unique_users
            - event_metrics.unique_users,
        realtime_activity,
    };

    Ok(Json(dashboard))
}
