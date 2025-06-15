pub mod command_handlers;
pub mod handlers;
pub mod stats;

use axum::Router;
use chrono::{DateTime, Utc};
use events_models::{Event, Metadata};
use serde::{Deserialize, Serialize};
use sql_connection::SqlConnect;
use utoipa::ToSchema;
use uuid::Uuid;

// Basic event request DTO matching the OpenAPI spec
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EventRequestDto {
    #[serde(rename = "userId")]
    pub user_id: Uuid,
    #[serde(rename = "eventType")]
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EventResponse {
    pub id: Uuid,
    #[serde(rename = "userId")]
    pub user_id: Uuid,
    #[serde(rename = "eventType")]
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<Metadata>,
}

impl From<Event> for EventResponse {
    fn from(event: Event) -> Self {
        Self {
            id: event.id,
            user_id: event.user_id,
            event_type: format!("event_type_{}", event.event_type_id), // This should be resolved from event_types table
            timestamp: event.timestamp,
            metadata: event.metadata,
        }
    }
}

pub use command_handlers::{
    BulkDeleteEventsError, BulkDeleteEventsHandler, CreateEventError,
    CreateEventHandler, DeleteEventError, DeleteEventHandler,
    UpdateEventError, UpdateEventHandler,
};
pub use handlers::*;
pub use stats::*;

pub fn event_routes() -> Router {
    let db = SqlConnect::from_global();
    let services = handlers::EventServices::new(db);
    handlers::EventHandlers::routes().with_state(services)
}
