use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;
use uuid::Uuid;

#[derive(
    Clone,
    Debug,
    PartialEq,
    DeriveEntityModel,
    Eq,
    Serialize,
    Deserialize,
    TypedBuilder,
)]
#[sea_orm(table_name = "events")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[builder(default)]
    pub id: Uuid,
    pub user_id: Uuid,
    pub event_type_id: i32,
    #[builder(default)]
    pub timestamp: DateTime<Utc>,
    #[sea_orm(column_type = "JsonBinary")]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::event_types::Entity",
        from = "Column::EventTypeId",
        to = "super::event_types::Column::Id"
    )]
    EventType,
}

impl Related<super::event_types::Entity> for Entity {
    fn to() -> RelationDef { Relation::EventType.def() }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, Serialize, Deserialize, TypedBuilder)]
pub struct CreateEventRequest {
    pub user_id: Uuid,
    pub event_type_id: i32,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TypedBuilder)]
pub struct UpdateEventRequest {
    pub event_type_id: Option<i32>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub event_type_id: i32,
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<serde_json::Value>,
}

impl From<Model> for EventResponse {
    fn from(event: Model) -> Self {
        Self {
            id: event.id,
            user_id: event.user_id,
            event_type_id: event.event_type_id,
            timestamp: event.timestamp,
            metadata: event.metadata,
        }
    }
}
