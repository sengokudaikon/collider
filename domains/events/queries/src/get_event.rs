use database_traits::dao::GenericDao;
use events_dao::EventDao;
use events_models::EventResponse;
use serde::Deserialize;
use sql_connection::SqlConnect;
use thiserror::Error;
use tracing::instrument;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum GetEventError {
    #[error("DAO error: {0}")]
    Dao(#[from] events_dao::EventDaoError),
    #[error("Event not found: {event_id}")]
    NotFound { event_id: Uuid },
}

#[derive(Debug, Deserialize)]
pub struct GetEventQuery {
    pub event_id: Uuid,
}

#[derive(Clone)]
pub struct GetEventQueryHandler {
    event_dao: EventDao,
}

impl GetEventQueryHandler {
    pub fn new(db: SqlConnect) -> Self {
        Self {
            event_dao: EventDao::new(db),
        }
    }

    #[instrument(skip(self))]
    pub async fn execute(
        &self, query: GetEventQuery,
    ) -> Result<EventResponse, GetEventError> {
        let event = self.event_dao.find_by_id(query.event_id).await.map_err(
            |_| {
                GetEventError::NotFound {
                    event_id: query.event_id,
                }
            },
        )?;

        Ok(event)
    }
}
