use std::net::SocketAddr;

use axum::{
    Router,
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
};
use redis_connection::{
    cache_provider::CacheProvider, config::RedisDbConfig, connect_redis_db,
    connection::RedisConnectionManager,
};
use sql_connection::{
    SqlConnect,
    config::{PostgresDbConfig, ReadReplicaConfig},
    connect_postgres_db, connect_postgres_read_replica,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use user_http::UserServices;
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
        max_conn: Some(600), // Match PostgreSQL config max_connections
        min_conn: Some(50),
        logger: false,
        // Read replica configuration
        read_replica_uri: std::env::var("DATABASE_READ_REPLICA_URL").ok(),
        read_max_conn: Some(1200), // Higher for read-heavy workloads
        read_min_conn: Some(100),
        enable_read_write_split: false,
    };

    // Initialize primary database connection
    connect_postgres_db(&db_config).await?;
    info!("PostgreSQL primary connection pool initialized");

    // Initialize read replica if configured
    if db_config.enable_read_write_split() {
        if let Err(e) = connect_postgres_read_replica(&db_config).await {
            warn!(
                "Failed to initialize read replica: {}. Continuing with \
                 primary only.",
                e
            );
        }
    }

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
    let user_services = UserServices::new(db.clone());
    let event_services = events_http::EventServices::new(db.clone());

    // Start background job for refreshing materialized views
    info!("Starting background job scheduler...");
    event_services.background_jobs.start().await;
    info!("Background job scheduler started successfully");

    let api_routes = Router::new()
        .route("/stats", axum::routing::get(events_http::stats::get_stats))
        .route(
            "/stats/refresh",
            axum::routing::post(events_http::stats::refresh_stats),
        )
        .route("/event", post(events_http::create_event))
        .route("/event/{id}", get(events_http::get_event))
        .route("/event/{id}", put(events_http::update_event))
        .route("/event/{id}", delete(events_http::delete_event))
        .route("/events", get(events_http::list_events))
        .route("/events", delete(events_http::bulk_delete_events))
        .with_state(event_services)
        .route("/user", post(user_http::create_user))
        .route("/user/{id}", get(user_http::get_user))
        .route("/user/{id}", put(user_http::update_user))
        .route("/user/{id}", delete(user_http::delete_user))
        .route("/user/{id}/events", get(user_http::get_user_events))
        .route("/users", get(user_http::list_users))
        .with_state(user_services.clone());

    let app = Router::new()
        .route("/", get(health_check))
        .merge(api_routes);

    let app = app
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
        events_http::stats::refresh_stats,
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
        (name = "users", description = "User management endpoints"),
        (name = "stats", description = "Event statistics endpoints")
    ),
    info(
        title = "Collider API",
        description = "High-performance event tracking API",
        version = "1.0.0"
    )
)]
struct ApiDoc;

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Health check successful with connection pool status", body = String)
    ),
    tag = "health"
)]
async fn health_check() -> impl IntoResponse {
    let db = SqlConnect::from_global();
    let (write_available, write_size, read_stats) = db.get_pool_status();

    let health_info = if let Some((read_available, read_size)) = read_stats {
        format!(
            "OK - Write Pool: {write_available}/{write_size} available, Read Pool: {read_available}/{read_size} available"
        )
    }
    else {
        format!(
            "OK - Single Pool: {write_available}/{write_size} available (Read replica not configured)"
        )
    };

    (StatusCode::OK, health_info)
}
