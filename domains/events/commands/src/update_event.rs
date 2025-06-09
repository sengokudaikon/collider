use database_traits::dao::GenericDao;
use events_dao::EventDao;
use events_models::{EventActiveModel, EventModel};
use sea_orm::ActiveValue::Set;
use serde::{Deserialize, Serialize};
use sql_connection::SqlConnect;
use thiserror::Error;
use tracing::instrument;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum UpdateEventError {
    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),
    #[error("DAO error: {0}")]
    Dao(#[from] events_dao::EventDaoError),
}

#[derive(Debug, Deserialize)]
pub struct UpdateEventCommand {
    #[serde(skip)]
    pub event_id: Uuid,
    pub event_type_id: Option<i32>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct UpdateEventResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub event_type_id: i32,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct UpdateEventResult {
    pub event: UpdateEventResponse,
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
        // Find the existing event to get current values
        let existing_event =
            self.event_dao.find_by_id(command.event_id).await?;

        // Create ActiveModel with updates
        let active_model = EventActiveModel {
            id: Set(existing_event.id),
            user_id: Set(existing_event.user_id),
            event_type_id: if let Some(event_type_id) = command.event_type_id
            {
                Set(event_type_id)
            }
            else {
                Set(existing_event.event_type_id)
            },
            timestamp: Set(existing_event.timestamp),
            metadata: if command.metadata.is_some() {
                Set(command.metadata)
            }
            else {
                Set(existing_event.metadata)
            },
        };

        let updated_event = self
            .event_dao
            .update(command.event_id, active_model)
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

impl From<EventModel> for UpdateEventResponse {
    fn from(event: EventModel) -> Self {
        Self {
            id: event.id,
            user_id: event.user_id,
            event_type_id: event.event_type_id,
            timestamp: event.timestamp,
            metadata: event.metadata,
        }
    }
}
