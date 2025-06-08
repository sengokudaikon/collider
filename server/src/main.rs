use std::net::SocketAddr;

use analytics_http::analytics_routes;
use axum::{Router, http::StatusCode, response::IntoResponse, routing::get};
use events_http::event_routes;
use redis_connection::{
    config::RedisDbConfig, connect_redis_db,
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
    RedisConnectionManager::init_static(redis_pool);
    info!("Redis connection pool initialized");

    info!("Connection pools initialized successfully");

    let db = SqlConnect::from_global();
    let user_services = UserServices::new(db.clone());

    let app = Router::new()
        .route("/health", get(health_check))
        .nest("/api/events", event_routes())
        .nest("/api/analytics", analytics_routes())
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

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    info!("🚀 Collider server starting on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[derive(OpenApi)]
#[openapi(
    paths(
        health_check
    ),
    components(
        schemas()
    ),
    tags(
        (name = "collider", description = "Collider API endpoints")
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
