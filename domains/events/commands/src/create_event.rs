use database_traits::dao::GenericDao;
use events_dao::EventDao;
use events_models::{CreateEventRequest, EventResponse};
use serde::{Deserialize, Serialize};
use sql_connection::SqlConnect;
use thiserror::Error;
use tracing::instrument;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum CreateEventError {
    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),
    #[error("DAO error: {0}")]
    Dao(#[from] events_dao::EventDaoError),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateEventCommand {
    pub user_id: Uuid,
    pub event_type_id: i32,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct CreateEventResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub event_type_id: i32,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct CreateEventResult {
    pub event: CreateEventResponse,
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
        let create_request = CreateEventRequest {
            user_id: command.user_id,
            event_type_id: command.event_type_id,
            metadata: command.metadata,
        };

        let saved_event = self.event_dao.create(create_request).await?;

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

impl From<EventResponse> for CreateEventResponse {
    fn from(event: EventResponse) -> Self {
        Self {
            id: event.id,
            user_id: event.user_id,
            event_type_id: event.event_type_id,
            timestamp: event.timestamp,
            metadata: event.metadata,
        }
    }
}
