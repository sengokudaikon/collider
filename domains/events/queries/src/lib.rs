use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct GetEventQuery {
    pub event_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct GetUserEventsQuery {
    pub user_id: i64,
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct ListEventsQuery {
    pub user_id: Option<i64>,
    pub event_type_id: Option<i32>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}
