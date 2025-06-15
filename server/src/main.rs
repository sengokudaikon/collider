use std::net::SocketAddr;

use analytics::RedisAnalyticsMetricsUpdater;
use analytics_http::AnalyticsHandlers;
use axum::{Router, http::StatusCode, response::IntoResponse, routing::get};
use events_http::EventHandlers;
use redis_connection::{
    cache_provider::CacheProvider, config::RedisDbConfig, connect_redis_db,
    connection::RedisConnectionManager,
};
use sql_connection::{
    SqlConnect, config::PostgresDbConfig, connect_postgres_db,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use user_http::{UserHandlers, UserServices};
use utoipa::OpenApi;
use utoipa_rapidoc::RapiDoc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Initializing connection pools...");

    let db_config = PostgresDbConfig {
        uri: std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgresql://postgres:postgres@localhost/postgres".to_string()
        }),
        max_conn: Some(100),
        min_conn: Some(20),
        logger: false,
    };
    connect_postgres_db(&db_config).await?;
    info!("PostgreSQL connection pool initialized");

    let redis_config = RedisDbConfig {
        host: std::env::var("REDIS_HOST")
            .unwrap_or_else(|_| "127.0.0.1".to_string()),
        port: std::env::var("REDIS_PORT")
            .unwrap_or_else(|_| "6379".to_string())
            .parse()
            .unwrap_or(6379),
        db: 0,
    };
    let redis_pool = connect_redis_db(&redis_config).await?;
    RedisConnectionManager::init_static(redis_pool.clone());
    CacheProvider::init_redis_static(redis_pool);
    info!("Redis connection pool and cache backend initialized");

    info!("Connection pools initialized successfully");

    let db = SqlConnect::from_global();
    let (user_services, _analytics_task) =
        UserServices::new_with_analytics(db.clone());

    let event_services = events_http::EventServices::new(db.clone());
    let analytics_services = analytics_http::AnalyticsServices::new(
        db.clone(),
        RedisAnalyticsMetricsUpdater::new(),
    );

    let app = Router::new()
        .route("/health", get(health_check))
        .nest(
            "/api/events",
            EventHandlers::routes().with_state(event_services),
        )
        .nest(
            "/api/analytics",
            AnalyticsHandlers::routes().with_state(analytics_services),
        )
        .nest(
            "/api/users",
            UserHandlers::routes().with_state(user_services),
        )
        .merge(RapiDoc::new("/api-docs/openapi.json").path("/docs"))
        .route(
            "/api-docs/openapi.json",
            get(|| async { axum::Json(ApiDoc::openapi()) }),
        )
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], 8880));
    info!("🚀 Collider server starting on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[derive(OpenApi)]
#[openapi(
    paths(
        health_check,
        events_http::create_event,
        events_http::update_event,
        events_http::delete_event,
        events_http::get_event,
        events_http::list_events,
        events_http::bulk_delete_events,
        events_http::stats::get_stats,
        analytics_http::get_realtime_metrics,
        analytics_http::get_hourly_summaries,
        analytics_http::get_user_activity,
        analytics_http::get_popular_events,
        user_http::create_user,
        user_http::update_user,
        user_http::delete_user,
        user_http::get_user,
        user_http::list_users,
        user_http::get_user_events
    ),
    components(
        schemas(
            events_responses::EventResponse,
            events_http::EventsListParams,
            events_http::EventsDeleteParams,
            events_http::stats::StatsQuery,
            events_http::stats::StatsResponse,
            events_commands::CreateEventCommand,
            events_commands::UpdateEventCommand,
            events_responses::BulkDeleteEventsResponse,
            user_responses::UserResponse,
            user_commands::CreateUserCommand,
            user_commands::UpdateUserCommand,
        )
    ),
    tags(
        (name = "health", description = "Health check endpoints"),
        (name = "events", description = "Event management endpoints"),
        (name = "analytics", description = "Analytics and metrics endpoints"),
        (name = "users", description = "User management endpoints")
    ),
    info(
        title = "Collider API",
        description = "High-performance event tracking and analytics API",
        version = "1.0.0"
    )
)]
struct ApiDoc;

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Health check successful", body = String)
    ),
    tag = "health"
)]
async fn health_check() -> impl IntoResponse { (StatusCode::OK, "OK") }
