pub mod handlers;

use axum::Router;
use chrono::{DateTime, Utc};
use events_models::EventModel;
use serde::{Deserialize, Serialize};
use sql_connection::SqlConnect;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub event_type_id: i32,
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<serde_json::Value>,
}

impl From<EventModel> for EventResponse {
    fn from(event: EventModel) -> Self {
        Self {
            id: event.id,
            user_id: event.user_id,
            event_type_id: event.event_type_id,
            timestamp: event.timestamp,
            metadata: event.metadata,
        }
    }
}

pub use handlers::*;

pub fn event_routes() -> Router {
    let db = SqlConnect::from_global();
    let services = handlers::EventServices::new(db);
    handlers::EventHandlers::routes().with_state(services)
}
