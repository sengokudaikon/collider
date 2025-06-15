use std::{collections::HashMap, sync::Arc};

use analytics::RedisAnalyticsMetricsUpdater;
use analytics_dao::AnalyticsViewsDao;
use analytics_models::{
    EventHourlySummary, EventMetrics, PageAnalytics, PopularEvent,
    ProductAnalytics, ReferrerAnalytics, UserDailyActivity, UserMetrics,
    UserSessionSummary,
};
use analytics_queries::{
    DashboardMetrics, EventMetricsQuery, HourlySummariesQuery,
    PageAnalyticsQuery, PopularEventsQuery, ProductAnalyticsQuery,
    RealtimeMetricsQuery, ReferrerAnalyticsQuery, RefreshViewsQuery,
    RefreshViewsResponse, UserActivityQuery, UserMetricsQuery,
    UserSessionSummariesQuery,
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
            .route("/views/user-sessions", get(get_user_session_summaries))
            .route("/views/page-analytics", get(get_page_analytics))
            .route("/views/product-analytics", get(get_product_analytics))
            .route("/views/referrer-analytics", get(get_referrer_analytics))
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

#[utoipa::path(
    get,
    path = "/api/analytics/views/user-sessions",
    params(UserSessionSummariesQuery),
    responses(
        (status = 200, description = "User session summaries", body = Vec<UserSessionSummary>),
        (status = 400, description = "Invalid query parameters"),
        (status = 500, description = "Internal server error")
    ),
    tag = "analytics-views"
)]
#[instrument(skip_all)]
pub async fn get_user_session_summaries(
    State(services): State<AnalyticsServices>,
    Query(params): Query<UserSessionSummariesQuery>,
) -> Result<Json<Vec<UserSessionSummary>>, AppError> {
    let summaries = services
        .dao
        .get_user_session_summaries(params.user_id, params.limit)
        .await?;

    Ok(Json(summaries))
}

#[utoipa::path(
    get,
    path = "/api/analytics/views/page-analytics",
    params(PageAnalyticsQuery),
    responses(
        (status = 200, description = "Page analytics", body = Vec<PageAnalytics>),
        (status = 400, description = "Invalid query parameters"),
        (status = 500, description = "Internal server error")
    ),
    tag = "analytics-views"
)]
#[instrument(skip_all)]
pub async fn get_page_analytics(
    State(services): State<AnalyticsServices>,
    Query(params): Query<PageAnalyticsQuery>,
) -> Result<Json<Vec<PageAnalytics>>, AppError> {
    let analytics = services
        .dao
        .get_page_analytics(
            params.page,
            params.start_time,
            params.end_time,
            params.limit,
        )
        .await?;

    Ok(Json(analytics))
}

#[utoipa::path(
    get,
    path = "/api/analytics/views/product-analytics",
    params(ProductAnalyticsQuery),
    responses(
        (status = 200, description = "Product analytics", body = Vec<ProductAnalytics>),
        (status = 400, description = "Invalid query parameters"),
        (status = 500, description = "Internal server error")
    ),
    tag = "analytics-views"
)]
#[instrument(skip_all)]
pub async fn get_product_analytics(
    State(services): State<AnalyticsServices>,
    Query(params): Query<ProductAnalyticsQuery>,
) -> Result<Json<Vec<ProductAnalytics>>, AppError> {
    let analytics = services
        .dao
        .get_product_analytics(
            params.product_id,
            params.event_type,
            params.start_date,
            params.end_date,
            params.limit,
        )
        .await?;

    Ok(Json(analytics))
}

#[utoipa::path(
    get,
    path = "/api/analytics/views/referrer-analytics",
    params(ReferrerAnalyticsQuery),
    responses(
        (status = 200, description = "Referrer analytics", body = Vec<ReferrerAnalytics>),
        (status = 400, description = "Invalid query parameters"),
        (status = 500, description = "Internal server error")
    ),
    tag = "analytics-views"
)]
#[instrument(skip_all)]
pub async fn get_referrer_analytics(
    State(services): State<AnalyticsServices>,
    Query(params): Query<ReferrerAnalyticsQuery>,
) -> Result<Json<Vec<ReferrerAnalytics>>, AppError> {
    let analytics = services
        .dao
        .get_referrer_analytics(
            params.referrer,
            params.start_date,
            params.end_date,
            params.limit,
        )
        .await?;

    Ok(Json(analytics))
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

#[cfg(test)]
mod tests {
    use analytics::RedisAnalyticsMetricsUpdater;
    use axum::{
        body::Body,
        http::{Method, Request, StatusCode},
        routing::Router,
    };
    use chrono::{DateTime, Duration, Utc};
    use redis_connection::cache_provider::CacheProvider;
    use serde_json::json;
    use test_utils::{
        postgres::TestPostgresContainer, redis::TestRedisContainer, *,
    };
    use tower::ServiceExt;
    use uuid::Uuid;

    use super::*;

    async fn setup_test_app()
    -> anyhow::Result<(TestPostgresContainer, Router)> {
        let container = TestPostgresContainer::new().await?;
        let redis_container = TestRedisContainer::new().await?;
        redis_container.flush_db().await?;

        CacheProvider::init_redis_static(redis_container.pool.clone());

        let sql_connect = create_sql_connect(&container);
        let redis_updater =
            RedisAnalyticsMetricsUpdater::new(redis_container.pool.clone());
        let services = AnalyticsServices::new(sql_connect, redis_updater);

        let app = AnalyticsHandlers::routes().with_state(services);

        Ok((container, app))
    }

    async fn create_test_analytics_data(
        container: &TestPostgresContainer,
    ) -> anyhow::Result<(Uuid, i32)> {
        let user_id = create_test_user(container).await?;
        let event_type_id = create_test_event_type(container).await?;

        // Create some test events for analytics
        for i in 0..3 {
            create_test_event(
                container,
                user_id,
                event_type_id,
                Some(&format!(
                    r#"{{"action": "test_{}", "value": {}}}"#,
                    i,
                    i * 10
                )),
            )
            .await?;
        }

        Ok((user_id, event_type_id))
    }

    #[tokio::test]
    async fn test_get_hourly_summaries_endpoint() {
        let (container, app) = setup_test_app().await.unwrap();
        let _ = create_test_analytics_data(&container).await.unwrap();

        let end_time = Utc::now();
        let start_time = end_time - Duration::hours(24);

        let request = Request::builder()
            .method(Method::GET)
            .uri(format!(
                "/views/hourly-summaries?start_time={}&end_time={}&limit=10",
                start_time.to_rfc3339(),
                end_time.to_rfc3339()
            ))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let response_json: serde_json::Value =
            serde_json::from_slice(&body).unwrap();

        assert!(response_json.is_array());
    }

    #[tokio::test]
    async fn test_get_user_activity_endpoint() {
        let (container, app) = setup_test_app().await.unwrap();
        let (user_id, _) =
            create_test_analytics_data(&container).await.unwrap();

        let end_date = Utc::now().date_naive();
        let start_date = end_date - Duration::days(7);

        let request = Request::builder()
            .method(Method::GET)
            .uri(format!(
                "/views/user-activity?user_id={}&start_date={}&end_date={}&\
                 limit=10",
                user_id,
                start_date.format("%Y-%m-%d"),
                end_date.format("%Y-%m-%d")
            ))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let response_json: serde_json::Value =
            serde_json::from_slice(&body).unwrap();

        assert!(response_json.is_array());
    }

    #[tokio::test]
    async fn test_get_popular_events_endpoint() {
        let (container, app) = setup_test_app().await.unwrap();
        let _ = create_test_analytics_data(&container).await.unwrap();

        let request = Request::builder()
            .method(Method::GET)
            .uri("/views/popular-events?period=day&limit=5")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let response_json: serde_json::Value =
            serde_json::from_slice(&body).unwrap();

        assert!(response_json.is_array());
    }

    #[tokio::test]
    async fn test_get_user_session_summaries_endpoint() {
        let (container, app) = setup_test_app().await.unwrap();
        let (user_id, _) =
            create_test_analytics_data(&container).await.unwrap();

        let request = Request::builder()
            .method(Method::GET)
            .uri(format!("/views/user-sessions?user_id={}&limit=10", user_id))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let response_json: serde_json::Value =
            serde_json::from_slice(&body).unwrap();

        assert!(response_json.is_array());
    }

    #[tokio::test]
    async fn test_get_page_analytics_endpoint() {
        let (container, app) = setup_test_app().await.unwrap();
        let _ = create_test_analytics_data(&container).await.unwrap();

        let end_time = Utc::now();
        let start_time = end_time - Duration::hours(24);

        let request = Request::builder()
            .method(Method::GET)
            .uri(format!(
                "/views/page-analytics?page=/home&start_time={}&end_time={}&\
                 limit=10",
                start_time.to_rfc3339(),
                end_time.to_rfc3339()
            ))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_product_analytics_endpoint() {
        let (container, app) = setup_test_app().await.unwrap();
        let _ = create_test_analytics_data(&container).await.unwrap();

        let end_date = Utc::now().date_naive();
        let start_date = end_date - Duration::days(7);

        let request = Request::builder()
            .method(Method::GET)
            .uri(format!(
                "/views/product-analytics?product_id=123&\
                 event_type=purchase&start_date={}&end_date={}&limit=10",
                start_date.format("%Y-%m-%d"),
                end_date.format("%Y-%m-%d")
            ))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_referrer_analytics_endpoint() {
        let (container, app) = setup_test_app().await.unwrap();
        let _ = create_test_analytics_data(&container).await.unwrap();

        let end_date = Utc::now().date_naive();
        let start_date = end_date - Duration::days(7);

        let request = Request::builder()
            .method(Method::GET)
            .uri(format!(
                "/views/referrer-analytics?referrer=google.com&\
                 start_date={}&end_date={}&limit=10",
                start_date.format("%Y-%m-%d"),
                end_date.format("%Y-%m-%d")
            ))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_refresh_views_endpoint() {
        let (container, app) = setup_test_app().await.unwrap();
        let _ = create_test_analytics_data(&container).await.unwrap();

        let request = Request::builder()
            .method(Method::POST)
            .uri("/views/refresh?view_name=hourly_summaries&concurrent=true")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let response_json: serde_json::Value =
            serde_json::from_slice(&body).unwrap();

        assert!(response_json.get("refreshed_views").is_some());
        assert!(response_json.get("duration_ms").is_some());
    }

    #[tokio::test]
    async fn test_get_event_metrics_endpoint() {
        let (container, app) = setup_test_app().await.unwrap();
        let _ = create_test_analytics_data(&container).await.unwrap();

        let end_time = Utc::now();
        let start_time = end_time - Duration::hours(24);

        let request = Request::builder()
            .method(Method::GET)
            .uri(format!(
                "/metrics/events?start={}&end={}",
                start_time.to_rfc3339(),
                end_time.to_rfc3339()
            ))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let response_json: serde_json::Value =
            serde_json::from_slice(&body).unwrap();

        assert!(response_json.get("total_events").is_some());
        assert!(response_json.get("unique_users").is_some());
    }

    #[tokio::test]
    async fn test_get_user_metrics_endpoint() {
        let (container, app) = setup_test_app().await.unwrap();
        let (user_id, _) =
            create_test_analytics_data(&container).await.unwrap();

        let end_time = Utc::now();
        let start_time = end_time - Duration::hours(24);

        let request = Request::builder()
            .method(Method::GET)
            .uri(format!(
                "/metrics/users/{}?start={}&end={}",
                user_id,
                start_time.to_rfc3339(),
                end_time.to_rfc3339()
            ))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let response_json: serde_json::Value =
            serde_json::from_slice(&body).unwrap();

        assert!(response_json.get("total_events").is_some());
        assert!(response_json.get("unique_sessions").is_some());
    }

    #[tokio::test]
    async fn test_get_realtime_metrics_endpoint() {
        let (container, app) = setup_test_app().await.unwrap();
        let _ = create_test_analytics_data(&container).await.unwrap();

        let timestamp = Utc::now();

        let request = Request::builder()
            .method(Method::GET)
            .uri(format!(
                "/metrics/realtime/hour?timestamp={}",
                timestamp.to_rfc3339()
            ))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let response_json: serde_json::Value =
            serde_json::from_slice(&body).unwrap();

        assert!(response_json.is_object());
    }

    #[tokio::test]
    async fn test_get_realtime_metrics_invalid_bucket() {
        let (container, app) = setup_test_app().await.unwrap();
        let _ = create_test_analytics_data(&container).await.unwrap();

        let request = Request::builder()
            .method(Method::GET)
            .uri("/metrics/realtime/invalid_bucket")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn test_get_dashboard_metrics_endpoint() {
        let (container, app) = setup_test_app().await.unwrap();
        let _ = create_test_analytics_data(&container).await.unwrap();

        let request = Request::builder()
            .method(Method::GET)
            .uri("/metrics/dashboard")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let response_json: serde_json::Value =
            serde_json::from_slice(&body).unwrap();

        assert!(response_json.get("total_events_today").is_some());
        assert!(response_json.get("unique_users_today").is_some());
        assert!(response_json.get("total_sessions_today").is_some());
        assert!(response_json.get("popular_events").is_some());
        assert!(response_json.get("realtime_activity").is_some());
    }

    #[tokio::test]
    async fn test_hourly_summaries_with_event_type_filter() {
        let (container, app) = setup_test_app().await.unwrap();
        let _ = create_test_analytics_data(&container).await.unwrap();

        let end_time = Utc::now();
        let start_time = end_time - Duration::hours(24);

        let request = Request::builder()
            .method(Method::GET)
            .uri(format!(
                "/views/hourly-summaries?start_time={}&end_time={}&\
                 event_types=test_event&limit=10",
                start_time.to_rfc3339(),
                end_time.to_rfc3339()
            ))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_event_metrics_with_type_filter() {
        let (container, app) = setup_test_app().await.unwrap();
        let _ = create_test_analytics_data(&container).await.unwrap();

        let end_time = Utc::now();
        let start_time = end_time - Duration::hours(24);

        let request = Request::builder()
            .method(Method::GET)
            .uri(format!(
                "/metrics/events?start={}&end={}&\
                 event_type_filter=test_event",
                start_time.to_rfc3339(),
                end_time.to_rfc3339()
            ))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
