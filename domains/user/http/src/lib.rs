pub mod analytics_integration;
pub mod command_handlers;
pub mod handlers;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SimpleEventResponse {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserResponse {
    pub id: Uuid,
    pub name: String,
    pub events: Vec<SimpleEventResponse>,
}

pub use user_queries::{
    GetUserByNameResponse,
};

impl From<user_models::User> for UserResponse {
    fn from(user: user_models::User) -> Self {
        Self {
            id: user.id,
            name: user.name,
            events: vec![],
        }
    }
}

impl From<GetUserByNameResponse> for UserResponse {
    fn from(response: GetUserByNameResponse) -> Self {
        Self {
            id: response.id,
            name: response.name,
            events: vec![],
        }
    }
}

impl UserResponse {
    pub fn with_event_ids(
        user: user_models::User, event_ids: Vec<Uuid>,
    ) -> Self {
        Self {
            id: user.id,
            name: user.name,
            events: event_ids
                .into_iter()
                .map(|id| SimpleEventResponse { id })
                .collect(),
        }
    }
}

pub use analytics_integration::{UserAnalyticsFactory, UserAnalyticsIntegration};
pub use command_handlers::{CreateUserHandler, UpdateUserHandler, DeleteUserHandler, CreateUserError, UpdateUserError, DeleteUserError};
pub use handlers::*;
