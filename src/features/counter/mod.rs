use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct CounterState {
    value: Arc<Mutex<i32>>,
}

impl CounterState {
    pub fn new() -> Self {
        Self {
            value: Arc::new(Mutex::new(0)),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct CounterResponse {
    value: i32,
}

#[derive(Deserialize)]
struct IncrementRequest {
    amount: Option<i32>,
}

async fn get_counter(State(state): State<CounterState>) -> Json<CounterResponse> {
    let counter = state.value.lock().await;
    Json(CounterResponse { value: *counter })
}

async fn increment_counter(
    State(state): State<CounterState>,
    Json(payload): Json<IncrementRequest>,
) -> (StatusCode, Json<CounterResponse>) {
    let amount = payload.amount.unwrap_or(1);
    let mut counter = state.value.lock().await;
    *counter += amount;

    (StatusCode::OK, Json(CounterResponse { value: *counter }))
}

pub fn routes() -> Router<CounterState> {
    Router::new()
        .route("/counter", get(get_counter))
        .route("/counter/increment", post(increment_counter))
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
    async fn test_get_counter() {
        let state = CounterState::new();
        let app = routes().with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/counter")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let counter_response: CounterResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(counter_response.value, 0);
    }

    #[tokio::test]
    async fn test_increment_counter() {
        let state = CounterState::new();
        let app = routes().with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/counter/increment")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"amount": 5}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let counter_response: CounterResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(counter_response.value, 5);
    }
}
