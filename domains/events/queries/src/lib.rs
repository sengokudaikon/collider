use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct GetEventQuery {
    pub event_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct GetUserEventsQuery {
    pub user_id: Uuid,
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct ListEventsQuery {
    pub user_id: Option<Uuid>,
    pub event_type_id: Option<i32>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}
