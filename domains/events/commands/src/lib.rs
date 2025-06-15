use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BulkDeleteEventsCommand {
    pub before: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateEventCommand {
    pub user_id: Uuid,
    pub event_type: String,
    pub timestamp: Option<DateTime<Utc>>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DeleteEventCommand {
    pub event_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateEventCommand {
    #[serde(skip)]
    pub event_id: Uuid,
    pub event_type_id: Option<i32>,
    pub metadata: Option<serde_json::Value>,
}
