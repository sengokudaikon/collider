use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    TypedBuilder,
    ToSchema,
)]
pub struct Event {
    #[builder(default)]
    pub id: Uuid,
    pub user_id: Uuid,
    pub event_type_id: i32,
    #[builder(default)]
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<serde_json::Value>,
}
