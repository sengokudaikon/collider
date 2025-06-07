use axum::Router;
use tower_http::cors::CorsLayer;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod features;

use features::counter::CounterState;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "collider=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let counter_state = CounterState::new();

    let app = Router::new()
        .route(
            "/",
            axum::routing::get(|| async { "Welcome to Collider API!" }),
        )
        .merge(features::health::routes())
        .merge(features::counter::routes().with_state(counter_state))
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    info!("Server running on http://0.0.0.0:3000");

    axum::serve(listener, app).await.unwrap();
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn test_root() {
        let counter_state = CounterState::new();
        let app = Router::new()
            .route(
                "/",
                axum::routing::get(|| async { "Welcome to Collider API!" }),
            )
            .merge(features::health::routes())
            .merge(features::counter::routes().with_state(counter_state));

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(&body[..], b"Welcome to Collider API!");
    }
}
