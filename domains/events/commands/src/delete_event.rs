use database_traits::dao::GenericDao;
use events_dao::EventDao;
use serde::Deserialize;
use sql_connection::SqlConnect;
use thiserror::Error;
use tracing::instrument;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum DeleteEventError {
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

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use database_traits::dao::GenericDao;
    use events_dao::{EventDao, EventDaoError};
    use events_models::NewEvent;
    use test_utils::{postgres::TestPostgresContainer, *};
    use uuid::Uuid;

    use super::*;

    async fn setup_test_db()
    -> anyhow::Result<(TestPostgresContainer, DeleteEventHandler, EventDao)>
    {
        let container = TestPostgresContainer::new().await?;

        let sql_connect = create_sql_connect(&container);
        let handler = DeleteEventHandler::new(sql_connect.clone());
        let dao = EventDao::new(sql_connect);

        Ok((container, handler, dao))
    }

    #[tokio::test]
    async fn test_delete_event_success() {
        let (container, handler, dao) = setup_test_db().await.unwrap();
        let event_type_id = create_test_event_type(&container).await.unwrap();
        let user_id = create_test_user(&container).await.unwrap();

        let new_event = NewEvent {
            id: Uuid::now_v7(),
            user_id,
            event_type_id,
            timestamp: Utc::now(),
            metadata: Some(serde_json::json!({"test": "data"})),
        };
        let created_event = dao.create(new_event).await.unwrap();

        let found_event = dao.find_by_id(created_event.id).await.unwrap();
        assert_eq!(found_event.id, created_event.id);

        let command = DeleteEventCommand {
            event_id: created_event.id,
        };
        let result = handler.execute(command).await;
        assert!(result.is_ok());

        let result = dao.find_by_id(created_event.id).await;
        assert!(matches!(result, Err(EventDaoError::NotFound)));
    }

    #[tokio::test]
    async fn test_delete_event_not_found() {
        let (_container, handler, _) = setup_test_db().await.unwrap();
        let non_existent_id = Uuid::now_v7();

        let command = DeleteEventCommand {
            event_id: non_existent_id,
        };

        let result = handler.execute(command).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_multiple_events() {
        let (container, handler, dao) = setup_test_db().await.unwrap();
        let event_type_id = create_test_event_type(&container).await.unwrap();
        let user_id = create_test_user(&container).await.unwrap();

        let mut event_ids = Vec::new();
        for i in 0..3 {
            let new_event = NewEvent {
                id: Uuid::now_v7(),
                user_id,
                event_type_id,
                timestamp: Utc::now(),
                metadata: Some(serde_json::json!({"sequence": i})),
            };
            let created_event = dao.create(new_event).await.unwrap();
            event_ids.push(created_event.id);
        }

        let command = DeleteEventCommand {
            event_id: event_ids[0],
        };
        handler.execute(command).await.unwrap();

        let result = dao.find_by_id(event_ids[0]).await;
        assert!(matches!(result, Err(EventDaoError::NotFound)));

        let second_event = dao.find_by_id(event_ids[1]).await.unwrap();
        assert_eq!(second_event.id, event_ids[1]);

        let third_event = dao.find_by_id(event_ids[2]).await.unwrap();
        assert_eq!(third_event.id, event_ids[2]);
    }

    #[tokio::test]
    async fn test_delete_event_with_complex_metadata() {
        let (container, handler, dao) = setup_test_db().await.unwrap();
        let event_type_id = create_test_event_type(&container).await.unwrap();
        let user_id = create_test_user(&container).await.unwrap();

        let complex_metadata = serde_json::json!({
            "nested": {
                "data": ["array", "values"],
                "number": 42,
                "boolean": true
            },
            "large_text": "a".repeat(1000)
        });

        let new_event = NewEvent {
            id: Uuid::now_v7(),
            user_id,
            event_type_id,
            timestamp: Utc::now(),
            metadata: Some(complex_metadata),
        };
        let created_event = dao.create(new_event).await.unwrap();

        let command = DeleteEventCommand {
            event_id: created_event.id,
        };
        let result = handler.execute(command).await;
        assert!(result.is_ok());

        let result = dao.find_by_id(created_event.id).await;
        assert!(matches!(result, Err(EventDaoError::NotFound)));
    }
}