use events_dao::EventDao;
use events_models::EventResponse;
use serde::Deserialize;
use sql_connection::SqlConnect;
use thiserror::Error;
use tracing::instrument;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ListEventsError {
    #[error("DAO error: {0}")]
    Dao(#[from] events_dao::EventDaoError),
}

#[derive(Debug, Deserialize)]
pub struct ListEventsQuery {
    pub user_id: Option<Uuid>,
    pub event_type_id: Option<i32>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

#[derive(Clone)]
pub struct ListEventsQueryHandler {
    event_dao: EventDao,
}

impl ListEventsQueryHandler {
    pub fn new(db: SqlConnect) -> Self {
        Self {
            event_dao: EventDao::new(db),
        }
    }

    #[instrument(skip(self))]
    pub async fn execute(
        &self, query: ListEventsQuery,
    ) -> Result<Vec<EventResponse>, ListEventsError> {
        let events = self
            .event_dao
            .find_with_filters(
                query.user_id,
                query.event_type_id,
                query.limit,
                query.offset,
            )
            .await?;
        Ok(events)
    }
}
