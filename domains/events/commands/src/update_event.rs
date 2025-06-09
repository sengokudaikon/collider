use database_traits::dao::GenericDao;
use events_dao::EventDao;
use events_models::{EventActiveModel, EventModel};
use sea_orm::ActiveValue::Set;
use serde::{Deserialize, Serialize};
use sql_connection::SqlConnect;
use thiserror::Error;
use tracing::instrument;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum UpdateEventError {
    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),
    #[error("DAO error: {0}")]
    Dao(#[from] events_dao::EventDaoError),
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateEventCommand {
    #[serde(skip)]
    pub event_id: Uuid,
    pub event_type_id: Option<i32>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, ToSchema)]
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

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use database_traits::dao::GenericDao;
    use events_dao::EventDao;
    use events_models::EventActiveModel;
    use sea_orm::ActiveValue::Set;
    use test_utils::{postgres::TestPostgresContainer, *};
    use uuid::Uuid;

    use super::*;

    async fn setup_test_db()
    -> anyhow::Result<(TestPostgresContainer, UpdateEventHandler, EventDao)>
    {
        let container = TestPostgresContainer::new().await?;

        let sql_connect = create_sql_connect(&container);
        let handler = UpdateEventHandler::new(sql_connect.clone());
        let dao = EventDao::new(sql_connect);

        Ok((container, handler, dao))
    }

    #[tokio::test]
    async fn test_update_event_metadata_only() {
        let (container, handler, dao) = setup_test_db().await.unwrap();
        let (event_type_id, _) =
            create_test_event_types(&container).await.unwrap();
        let user_id = create_test_user(&container).await.unwrap();

        let create_request = EventActiveModel {
            id: Set(Uuid::now_v7()),
            user_id: Set(user_id),
            event_type_id: Set(event_type_id),
            timestamp: Set(Utc::now()),
            metadata: Set(Some(serde_json::json!({"original": "data"}))),
        };
        let created_event = dao.create(create_request).await.unwrap();

        let command = UpdateEventCommand {
            event_id: created_event.id,
            event_type_id: None,
            metadata: Some(serde_json::json!({"updated": "metadata"})),
        };

        let result = handler.execute(command).await.unwrap();

        assert_eq!(result.event.id, created_event.id);
        assert_eq!(result.event.event_type_id, event_type_id);
        assert_eq!(
            result.event.metadata,
            Some(serde_json::json!({"updated": "metadata"}))
        );
    }

    #[tokio::test]
    async fn test_update_event_type_only() {
        let (container, handler, dao) = setup_test_db().await.unwrap();
        let (original_type, new_type) =
            create_test_event_types(&container).await.unwrap();
        let user_id = create_test_user(&container).await.unwrap();

        let create_request = EventActiveModel {
            id: Set(Uuid::now_v7()),
            user_id: Set(user_id),
            event_type_id: Set(original_type),
            timestamp: Set(Utc::now()),
            metadata: Set(Some(serde_json::json!({"key": "value"}))),
        };
        let created_event = dao.create(create_request).await.unwrap();

        let command = UpdateEventCommand {
            event_id: created_event.id,
            event_type_id: Some(new_type),
            metadata: None,
        };

        let result = handler.execute(command).await.unwrap();

        assert_eq!(result.event.id, created_event.id);
        assert_eq!(result.event.event_type_id, new_type);
        assert_eq!(
            result.event.metadata,
            Some(serde_json::json!({"key": "value"}))
        );
    }

    #[tokio::test]
    async fn test_update_event_both_fields() {
        let (container, handler, dao) = setup_test_db().await.unwrap();
        let (original_type, new_type) =
            create_test_event_types(&container).await.unwrap();
        let user_id = create_test_user(&container).await.unwrap();

        let create_request = EventActiveModel {
            id: Set(Uuid::now_v7()),
            user_id: Set(user_id),
            event_type_id: Set(original_type),
            timestamp: Set(Utc::now()),
            metadata: Set(Some(serde_json::json!({"original": "data"}))),
        };
        let created_event = dao.create(create_request).await.unwrap();

        let command = UpdateEventCommand {
            event_id: created_event.id,
            event_type_id: Some(new_type),
            metadata: Some(
                serde_json::json!({"completely": "new", "data": "here"}),
            ),
        };

        let result = handler.execute(command).await.unwrap();

        assert_eq!(result.event.id, created_event.id);
        assert_eq!(result.event.event_type_id, new_type);
        assert_eq!(
            result.event.metadata,
            Some(serde_json::json!({"completely": "new", "data": "here"}))
        );
    }

    #[tokio::test]
    async fn test_update_event_clear_metadata() {
        let (container, handler, dao) = setup_test_db().await.unwrap();
        let (event_type_id, _) =
            create_test_event_types(&container).await.unwrap();
        let user_id = create_test_user(&container).await.unwrap();

        let create_request = EventActiveModel {
            id: Set(Uuid::now_v7()),
            user_id: Set(user_id),
            event_type_id: Set(event_type_id),
            timestamp: Set(Utc::now()),
            metadata: Set(Some(serde_json::json!({"original": "data"}))),
        };
        let created_event = dao.create(create_request).await.unwrap();

        let command = UpdateEventCommand {
            event_id: created_event.id,
            event_type_id: None,
            metadata: Some(serde_json::Value::Null),
        };

        let result = handler.execute(command).await.unwrap();

        assert_eq!(result.event.id, created_event.id);
        assert!(
            result.event.metadata.is_none()
                || result.event.metadata == Some(serde_json::Value::Null)
        );
    }

    #[tokio::test]
    async fn test_update_event_not_found() {
        let (container, handler, _) = setup_test_db().await.unwrap();
        let (_, new_type) =
            create_test_event_types(&container).await.unwrap();
        let non_existent_id = Uuid::now_v7();

        let command = UpdateEventCommand {
            event_id: non_existent_id,
            event_type_id: Some(new_type),
            metadata: Some(serde_json::json!({"test": "data"})),
        };

        let result = handler.execute(command).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_event_invalid_event_type() {
        let (container, handler, dao) = setup_test_db().await.unwrap();
        let (event_type_id, _) =
            create_test_event_types(&container).await.unwrap();
        let user_id = create_test_user(&container).await.unwrap();

        let create_request = EventActiveModel {
            id: Set(Uuid::now_v7()),
            user_id: Set(user_id),
            event_type_id: Set(event_type_id),
            timestamp: Set(Utc::now()),
            metadata: Set(None),
        };
        let created_event = dao.create(create_request).await.unwrap();

        let command = UpdateEventCommand {
            event_id: created_event.id,
            event_type_id: Some(999),
            metadata: None,
        };

        let result = handler.execute(command).await;
        assert!(result.is_err());
    }
}
