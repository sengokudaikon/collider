use database_traits::dao::GenericDao;
use events_dao::EventDao;
use serde::Deserialize;
use sql_connection::SqlConnect;
use thiserror::Error;
use tracing::instrument;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum DeleteEventError {
    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),
    #[error("DAO error: {0}")]
    Dao(#[from] events_dao::EventDaoError),
}

#[derive(Debug, Deserialize)]
pub struct DeleteEventCommand {
    pub event_id: Uuid,
}

#[derive(Clone)]
pub struct DeleteEventHandler {
    event_dao: EventDao,
}

impl DeleteEventHandler {
    pub fn new(db: SqlConnect) -> Self {
        Self {
            event_dao: EventDao::new(db),
        }
    }

    #[instrument(skip(self))]
    pub async fn execute(
        &self, command: DeleteEventCommand,
    ) -> Result<(), DeleteEventError> {
        self.event_dao.delete(command.event_id).await?;
        Ok(())
    }
}
