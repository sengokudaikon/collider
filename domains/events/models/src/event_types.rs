use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    TypedBuilder,
)]
pub struct EventType {
    #[builder(default)]
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewEventType {
    pub name: String,
}

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

impl From<EventType> for EventTypeResponse {
    fn from(event_type: EventType) -> Self {
        Self {
            id: event_type.id,
            name: event_type.name,
        }
    }
}