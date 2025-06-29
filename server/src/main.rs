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
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};
use user_http::UserServices;
use utoipa::OpenApi;
use utoipa_rapidoc::RapiDoc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    // Configure logging with both console and file output
    let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into());
    let enable_file_logging = std::env::var("LOG_TO_FILE").unwrap_or_else(|_| "true".into()) == "true";

    let env_filter = tracing_subscriber::EnvFilter::new(&log_level);
    
    let registry = tracing_subscriber::registry().with(env_filter);

    if enable_file_logging {
        // File appender for clean, structured logs
        let file_appender = tracing_appender::rolling::daily("./logs", "collider.log");
        let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
        
        // Clean file format (no colors, structured)
        let file_layer = fmt::layer()
            .with_writer(non_blocking)
            .with_ansi(false)
            .with_target(true)
            .with_thread_ids(false)
            .with_line_number(false)
            .with_file(false)
            .compact();

        // Console format (with colors for development)
        let console_layer = fmt::layer()
            .with_writer(std::io::stdout)
            .with_ansi(true)
            .with_target(false)
            .compact();

        registry
            .with(file_layer)
            .with(console_layer)
            .init();
            
        // Keep the guard alive for the duration of the program
        std::mem::forget(_guard);
    } else {
        // Console only
        let console_layer = fmt::layer()
            .with_writer(std::io::stdout)
            .with_ansi(true)
            .with_target(false)
            .compact();

        registry.with(console_layer).init();
    }

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
    
    // Test actual connectivity instead of relying on deadpool status
    match db.get_client().await {
        Ok(_) => {
            let health_info = "OK - Database connection pool operational (deadpool status reporting disabled due to known issues)";
            (StatusCode::OK, health_info.to_string())
        }
        Err(e) => {
            let error_info = format!("ERROR - Database connection failed: {}", e);
            (StatusCode::SERVICE_UNAVAILABLE, error_info)
        }
    }
}
