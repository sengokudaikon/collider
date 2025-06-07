pub mod analytics_handlers;
pub mod handlers;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Minimal event representation for efficient responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleEventResponse {
    pub id: Uuid,
}

/// HTTP response model for user data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub username: String,
    pub events: Vec<SimpleEventResponse>,
    /// Aggregated event metrics for efficient querying
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<UserEventMetrics>,
}

// Re-export from user_queries for consistency
pub use user_queries::{
    EventTypeCount, GetUserByNameResponse, UserEventMetrics,
};

impl From<user_models::Model> for UserResponse {
    fn from(user: user_models::Model) -> Self {
        Self {
            id: user.id,
            username: user.name,
            events: vec![],
            metrics: None,
        }
    }
}

impl From<GetUserByNameResponse> for UserResponse {
    fn from(response: GetUserByNameResponse) -> Self {
        Self {
            id: response.id,
            username: response.name,
            events: vec![],
            metrics: None,
        }
    }
}

/// Constructor for UserResponse with aggregated metrics
impl UserResponse {
    pub fn with_metrics(
        user: user_models::Model, metrics: UserEventMetrics,
    ) -> Self {
        Self {
            id: user.id,
            username: user.name,
            events: vec![], // Skip individual events when showing metrics
            metrics: Some(metrics),
        }
    }

    pub fn with_event_ids(
        user: user_models::Model, event_ids: Vec<Uuid>,
    ) -> Self {
        Self {
            id: user.id,
            username: user.name,
            events: event_ids
                .into_iter()
                .map(|id| SimpleEventResponse { id })
                .collect(),
            metrics: None,
        }
    }
}

pub use handlers::*;
