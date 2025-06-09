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

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use test_utils::{postgres::TestPostgresContainer, *};
    use uuid::Uuid;

    use super::*;

    async fn setup_test_db()
    -> anyhow::Result<(TestPostgresContainer, CreateEventHandler)> {
        let container = TestPostgresContainer::new().await?;

        let sql_connect = create_sql_connect(&container);
        let handler = CreateEventHandler::new(sql_connect);

        Ok((container, handler))
    }

    #[tokio::test]
    async fn test_create_event_success() {
        let (container, handler) = setup_test_db().await.unwrap();
        let event_type_id = create_test_event_type(&container).await.unwrap();
        let user_id = create_test_user(&container).await.unwrap();

        let command = CreateEventCommand {
            user_id,
            event_type: "test_event".to_string(),
            timestamp: None,
            metadata: Some(serde_json::json!({"key": "value"})),
        };

        let result = handler.execute(command).await.unwrap();

        assert_eq!(result.event.user_id, user_id);
        assert_eq!(result.event.event_type_id, event_type_id);
        assert!(result.event.metadata.is_some());
        assert!(result.event.timestamp <= Utc::now());
    }

    #[tokio::test]
    async fn test_create_event_without_metadata() {
        let (container, handler) = setup_test_db().await.unwrap();
        let event_type_id = create_test_event_type(&container).await.unwrap();
        let user_id = create_test_user(&container).await.unwrap();

        let command = CreateEventCommand {
            user_id,
            event_type: "test_event".to_string(),
            timestamp: None,
            metadata: None,
        };

        let result = handler.execute(command).await.unwrap();

        assert_eq!(result.event.user_id, user_id);
        assert_eq!(result.event.event_type_id, event_type_id);
        assert!(result.event.metadata.is_none());
    }

    #[tokio::test]
    async fn test_create_event_with_complex_metadata() {
        let (container, handler) = setup_test_db().await.unwrap();
        create_test_event_type(&container).await.unwrap();
        let user_id = create_test_user(&container).await.unwrap();

        let complex_metadata = serde_json::json!({
            "page": "home",
            "action": "click",
            "element": {
                "type": "button",
                "id": "submit-btn",
                "text": "Submit"
            },
            "timestamp": "2023-01-01T12:00:00Z",
            "user_agent": "Mozilla/5.0",
            "session_id": "abc123"
        });

        let command = CreateEventCommand {
            user_id,
            event_type: "test_event".to_string(),
            timestamp: None,
            metadata: Some(complex_metadata.clone()),
        };

        let result = handler.execute(command).await.unwrap();

        assert_eq!(result.event.metadata, Some(complex_metadata));
    }

    #[tokio::test]
    async fn test_create_event_invalid_event_type() {
        let (container, handler) = setup_test_db().await.unwrap();
        let user_id = create_test_user(&container).await.unwrap();

        let command = CreateEventCommand {
            user_id,
            event_type: "non_existent_event_type".to_string(),
            timestamp: None,
            metadata: None,
        };

        let result = handler.execute(command).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_event_invalid_user() {
        let (container, handler) = setup_test_db().await.unwrap();
        create_test_event_type(&container).await.unwrap();
        let invalid_user_id = Uuid::now_v7();

        let command = CreateEventCommand {
            user_id: invalid_user_id,
            event_type: "test_event".to_string(),
            timestamp: None,
            metadata: None,
        };

        let result = handler.execute(command).await;
        assert!(result.is_err());
    }
}
