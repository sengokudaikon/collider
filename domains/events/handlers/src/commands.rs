use database_traits::dao::GenericDao;
use events_commands::{
    BulkDeleteEventsCommand, CreateEventCommand, DeleteEventCommand,
    UpdateEventCommand,
};
use events_dao::{EventDao, EventTypeDao};
use events_errors::EventError;
use events_responses::{BulkDeleteEventsResponse, EventResponse};
use sql_connection::SqlConnect;
use tracing::instrument;

#[derive(Clone)]
pub struct CreateEventHandler {
    event_dao: EventDao,
    event_type_dao: EventTypeDao,
}

impl CreateEventHandler {
    pub fn new(db: SqlConnect) -> Self {
        Self {
            event_dao: EventDao::new(db.clone()),
            event_type_dao: EventTypeDao::new(db),
        }
    }

    #[instrument(skip(self))]
    pub async fn execute(
        &self, command: CreateEventCommand,
    ) -> Result<EventResponse, EventError> {
        let saved_event = self.event_dao.create(command).await?;
        let event_type = self
            .event_type_dao
            .find_by_id(saved_event.event_type_id)
            .await
            .map(|et| et.name)
            .unwrap_or_else(|_| String::new());

        Ok(EventResponse {
            id: saved_event.id,
            user_id: saved_event.user_id,
            event_type,
            event_type_id: saved_event.event_type_id,
            timestamp: saved_event.timestamp,
            metadata: saved_event.metadata,
        })
    }
}

#[derive(Clone)]
pub struct UpdateEventHandler {
    event_dao: EventDao,
    event_type_dao: EventTypeDao,
}

impl UpdateEventHandler {
    pub fn new(db: SqlConnect) -> Self {
        Self {
            event_dao: EventDao::new(db.clone()),
            event_type_dao: EventTypeDao::new(db),
        }
    }

    #[instrument(skip(self))]
    pub async fn execute(
        &self, command: UpdateEventCommand,
    ) -> Result<EventResponse, EventError> {
        let updated_event =
            self.event_dao.update(command.event_id, command).await?;
        let event_type = self
            .event_type_dao
            .find_by_id(updated_event.event_type_id)
            .await
            .map(|et| et.name)
            .unwrap_or_else(|_| String::new());

        Ok(EventResponse {
            id: updated_event.id,
            user_id: updated_event.user_id,
            event_type,
            event_type_id: updated_event.event_type_id,
            timestamp: updated_event.timestamp,
            metadata: updated_event.metadata,
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
    ) -> Result<(), EventError> {
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
    ) -> Result<BulkDeleteEventsResponse, EventError> {
        let deleted_count = self
            .event_dao
            .delete_before_timestamp(command.before)
            .await?;

        Ok(BulkDeleteEventsResponse {
            deleted_count,
            deleted_before: command.before,
        })
    }
}
