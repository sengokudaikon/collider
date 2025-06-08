pub mod analytics_handlers;
pub mod handlers;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleEventResponse {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub name: String,
    pub events: Vec<SimpleEventResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<UserEventMetrics>,
}

pub use user_queries::{
    EventTypeCount, GetUserByNameResponse, UserEventMetrics,
};

impl From<user_models::Model> for UserResponse {
    fn from(user: user_models::Model) -> Self {
        Self {
            id: user.id,
            name: user.name,
            events: vec![],
            metrics: None,
        }
    }
}

impl From<GetUserByNameResponse> for UserResponse {
    fn from(response: GetUserByNameResponse) -> Self {
        Self {
            id: response.id,
            name: response.name,
            events: vec![],
            metrics: None,
        }
    }
}

impl UserResponse {
    pub fn with_metrics(
        user: user_models::Model, metrics: UserEventMetrics,
    ) -> Self {
        Self {
            id: user.id,
            name: user.name,
            events: vec![],
            metrics: Some(metrics),
        }
    }

    pub fn with_event_ids(
        user: user_models::Model, event_ids: Vec<Uuid>,
    ) -> Self {
        Self {
            id: user.id,
            name: user.name,
            events: event_ids
                .into_iter()
                .map(|id| SimpleEventResponse { id })
                .collect(),
            metrics: None,
        }
    }
}

pub use handlers::*;
