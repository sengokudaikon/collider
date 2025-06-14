use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BulkDeleteEventsCommand {
    pub before: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BulkDeleteEventsResponse {
    pub deleted_count: u64,
    pub deleted_before: DateTime<Utc>,
}
