use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

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
#[sea_orm(table_name = "event_types")]
pub struct Model {
    #[sea_orm(primary_key)]
    #[builder(default)]
    pub id: i32,
    pub name: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::events::Entity")]
    Events,
}

impl Related<super::events::Entity> for Entity {
    fn to() -> RelationDef { Relation::Events.def() }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, Serialize, Deserialize, TypedBuilder)]
pub struct CreateEventTypeRequest {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TypedBuilder)]
pub struct UpdateEventTypeRequest {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventTypeResponse {
    pub id: i32,
    pub name: String,
}

impl From<Model> for EventTypeResponse {
    fn from(event_type: Model) -> Self {
        Self {
            id: event_type.id,
            name: event_type.name,
        }
    }
}
