use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;
use utoipa::ToSchema;

use crate::Metadata;

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
    pub id: i64,
    pub user_id: i64,
    pub event_type_id: i32,
    #[builder(default)]
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<Metadata>,
}
