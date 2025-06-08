use events_dao::EventDao;
use events_models::EventResponse;
use serde::Deserialize;
use sql_connection::SqlConnect;
use thiserror::Error;
use tracing::instrument;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum GetUserEventsError {
    #[error("DAO error: {0}")]
    Dao(#[from] events_dao::EventDaoError),
}

#[derive(Debug, Deserialize)]
pub struct GetUserEventsQuery {
    pub user_id: Uuid,
    pub limit: Option<u64>,
}

#[derive(Clone)]
pub struct GetUserEventsQueryHandler {
    event_dao: EventDao,
}

impl GetUserEventsQueryHandler {
    pub fn new(db: SqlConnect) -> Self {
        Self {
            event_dao: EventDao::new(db),
        }
    }

    #[instrument(skip(self))]
    pub async fn execute(
        &self, query: GetUserEventsQuery,
    ) -> Result<Vec<EventResponse>, GetUserEventsError> {
        let events = self
            .event_dao
            .find_by_user_id(query.user_id, query.limit)
            .await?;
        Ok(events)
    }
}
