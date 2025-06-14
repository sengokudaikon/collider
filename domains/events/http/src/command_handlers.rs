use database_traits::dao::GenericDao;
use events_commands::{
    BulkDeleteEventsCommand, BulkDeleteEventsResponse,
    CreateEventCommand, CreateEventResponse, CreateEventResult,
    DeleteEventCommand, UpdateEventCommand, UpdateEventResponse, UpdateEventResult,
};
use events_dao::{EventDao, EventDaoError, EventTypeDaoError};
use sql_connection::SqlConnect;
use thiserror::Error;
use tracing::instrument;

#[derive(Debug, Error)]
pub enum CreateEventError {
    #[error("Event DAO error: {0}")]
    EventDao(#[from] EventDaoError),
    #[error("Event type DAO error: {0}")]
    EventTypeDao(#[from] EventTypeDaoError),
}

#[derive(Debug, Error)]
pub enum UpdateEventError {
    #[error("DAO error: {0}")]
    Dao(#[from] EventDaoError),
}

#[derive(Debug, Error)]
pub enum DeleteEventError {
    #[error("DAO error: {0}")]
    Dao(#[from] EventDaoError),
}

#[derive(Debug, Error)]
pub enum BulkDeleteEventsError {
    #[error("Event DAO error: {0}")]
    EventDao(#[from] EventDaoError),
}

#[derive(Clone)]
pub struct CreateEventHandler {
    event_dao: EventDao,
}

impl CreateEventHandler {
    pub fn new(db: SqlConnect) -> Self {
        Self {
            event_dao: EventDao::new(db),
        }
    }

    #[instrument(skip(self))]
    pub async fn execute(
        &self, command: CreateEventCommand,
    ) -> Result<CreateEventResult, CreateEventError> {
        let saved_event = self.event_dao.create(command).await?;

        Ok(CreateEventResult {
            event: CreateEventResponse {
                id: saved_event.id,
                user_id: saved_event.user_id,
                event_type_id: saved_event.event_type_id,
                timestamp: saved_event.timestamp,
                metadata: saved_event.metadata,
            },
        })
    }
}

#[derive(Clone)]
pub struct UpdateEventHandler {
    event_dao: EventDao,
}

impl UpdateEventHandler {
    pub fn new(db: SqlConnect) -> Self {
        Self {
            event_dao: EventDao::new(db),
        }
    }

    #[instrument(skip(self))]
    pub async fn execute(
        &self, command: UpdateEventCommand,
    ) -> Result<UpdateEventResult, UpdateEventError> {
        let updated_event = self
            .event_dao
            .update(command.event_id, command)
            .await?;

        Ok(UpdateEventResult {
            event: UpdateEventResponse {
                id: updated_event.id,
                user_id: updated_event.user_id,
                event_type_id: updated_event.event_type_id,
                timestamp: updated_event.timestamp,
                metadata: updated_event.metadata,
            },
        })
    }
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

#[derive(Clone)]
pub struct BulkDeleteEventsHandler {
    event_dao: EventDao,
}

impl BulkDeleteEventsHandler {
    pub fn new(db: SqlConnect) -> Self {
        Self {
            event_dao: EventDao::new(db),
        }
    }

    #[instrument(skip(self))]
    pub async fn execute(
        &self, command: BulkDeleteEventsCommand,
    ) -> Result<BulkDeleteEventsResponse, BulkDeleteEventsError> {
        let deleted_count = self.event_dao.delete_before_timestamp(command.before).await?;

        Ok(BulkDeleteEventsResponse {
            deleted_count,
            deleted_before: command.before,
        })
    }
}