use chrono::Utc;
use database_traits::dao::GenericDao;
use events_dao::{EventDao, EventTypeDao};
use events_models::{EventActiveModel, EventModel};
use sea_orm::ActiveValue::Set;
use serde::{Deserialize, Serialize};
use sql_connection::SqlConnect;
use thiserror::Error;
use tracing::instrument;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum CreateEventError {
    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),
    #[error("Event DAO error: {0}")]
    EventDao(#[from] events_dao::EventDaoError),
    #[error("Event type DAO error: {0}")]
    EventTypeDao(#[from] events_dao::EventTypeDaoError),
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateEventCommand {
    pub user_id: Uuid,
    pub event_type: String,
    pub timestamp: Option<chrono::DateTime<chrono::Utc>>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, ToSchema)]
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
    ) -> Result<CreateEventResult, CreateEventError> {
        let event_type = self
            .event_type_dao
            .find_by_name(&command.event_type)
            .await?;

        let active_model = EventActiveModel {
            id: Set(Uuid::now_v7()),
            user_id: Set(command.user_id),
            event_type_id: Set(event_type.id),
            timestamp: Set(command.timestamp.unwrap_or_else(Utc::now)),
            metadata: Set(command.metadata),
        };

        let saved_event = self.event_dao.create(active_model).await?;

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

impl From<EventModel> for CreateEventResponse {
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
