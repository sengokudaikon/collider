use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct BulkDeleteEventsResponse {
    pub deleted_count: u64,
    pub deleted_before: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EventResponse {
    pub id: i64,
    #[serde(rename = "userId")]
    pub user_id: i64,
    #[serde(rename = "eventType")]
    pub event_type: String,
    pub event_type_id: i32,
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<events_models::Metadata>,
}

impl From<events_models::Event> for EventResponse {
    fn from(event: events_models::Event) -> Self {
        Self {
            id: event.id,
            user_id: event.user_id,
            event_type_id: event.event_type_id,
            event_type: format!("event_type_{}", event.event_type_id), /* This should be resolved from event_types table */
            timestamp: event.timestamp,
            metadata: event.metadata,
        }
    }
}
