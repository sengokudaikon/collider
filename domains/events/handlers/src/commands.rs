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

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};
    use database_traits::dao::GenericDao;
    use serde_json::json;
    use test_utils::{TestPostgresContainer, *};
    use uuid::Uuid;

    use super::*;

    async fn setup_test_handlers() -> anyhow::Result<(
        TestPostgresContainer,
        CreateEventHandler,
        UpdateEventHandler,
        DeleteEventHandler,
        BulkDeleteEventsHandler,
    )> {
        let container = TestPostgresContainer::new().await?;
        let sql_connect = create_sql_connect(&container);

        let create_handler = CreateEventHandler::new(sql_connect.clone());
        let update_handler = UpdateEventHandler::new(sql_connect.clone());
        let delete_handler = DeleteEventHandler::new(sql_connect.clone());
        let bulk_delete_handler = BulkDeleteEventsHandler::new(sql_connect);

        Ok((
            container,
            create_handler,
            update_handler,
            delete_handler,
            bulk_delete_handler,
        ))
    }

    #[tokio::test]
    async fn test_create_event_handler() {
        let (container, create_handler, ..) =
            setup_test_handlers().await.unwrap();

        let event_type_id = create_test_event_type(&container).await.unwrap();
        let user_id = create_test_user(&container).await.unwrap();

        let command = CreateEventCommand {
            user_id,
            event_type: "test_event".to_string(),
            timestamp: Some(Utc::now()),
            metadata: Some(json!({"action": "click", "button": "submit"})),
        };

        let result = create_handler.execute(command).await.unwrap();

        assert_eq!(result.user_id, user_id);
        assert_eq!(result.event_type, "test_event");
        assert_eq!(result.event_type_id, event_type_id);
        assert_ne!(result.id, Uuid::nil());
        assert!(result.metadata.is_some());
    }

    #[tokio::test]
    async fn test_create_event_handler_invalid_event_type() {
        let (container, create_handler, ..) =
            setup_test_handlers().await.unwrap();

        let user_id = create_test_user(&container).await.unwrap();

        let command = CreateEventCommand {
            user_id,
            event_type: "non_existent_event_type".to_string(),
            timestamp: Some(Utc::now()),
            metadata: None,
        };

        let result = create_handler.execute(command).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_event_handler_invalid_user() {
        let (container, create_handler, ..) =
            setup_test_handlers().await.unwrap();

        let _event_type_id =
            create_test_event_type(&container).await.unwrap();
        let non_existent_user_id = Uuid::now_v7();

        let command = CreateEventCommand {
            user_id: non_existent_user_id,
            event_type: "test_event".to_string(),
            timestamp: Some(Utc::now()),
            metadata: None,
        };

        let result = create_handler.execute(command).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_event_handler_with_null_timestamp() {
        let (container, create_handler, ..) =
            setup_test_handlers().await.unwrap();

        let _event_type_id =
            create_test_event_type(&container).await.unwrap();
        let user_id = create_test_user(&container).await.unwrap();

        let command = CreateEventCommand {
            user_id,
            event_type: "test_event".to_string(),
            timestamp: None, // Should use current timestamp
            metadata: Some(json!({"test": "data"})),
        };

        let result = create_handler.execute(command).await.unwrap();

        assert_eq!(result.user_id, user_id);
        assert_eq!(result.event_type, "test_event");
        // Timestamp should be set automatically
        assert!(result.timestamp > Utc::now() - Duration::seconds(10));
    }

    #[tokio::test]
    async fn test_update_event_handler() {
        let (container, create_handler, update_handler, ..) =
            setup_test_handlers().await.unwrap();

        let _event_type_id =
            create_test_event_type(&container).await.unwrap();
        let user_id = create_test_user(&container).await.unwrap();

        // Create a new event type for update
        container
            .execute_sql(
                "INSERT INTO event_types (id, name) VALUES (2, \
                 'updated_event')",
            )
            .await
            .unwrap();

        // First create an event
        let create_command = CreateEventCommand {
            user_id,
            event_type: "test_event".to_string(),
            timestamp: Some(Utc::now()),
            metadata: Some(json!({"original": "data"})),
        };
        let created_event =
            create_handler.execute(create_command).await.unwrap();

        // Then update it
        let update_command = UpdateEventCommand {
            event_id: created_event.id,
            event_type_id: Some(2),
            metadata: Some(json!({"updated": "metadata"})),
            timestamp: None,
        };

        let result = update_handler.execute(update_command).await.unwrap();

        assert_eq!(result.id, created_event.id);
        assert_eq!(result.user_id, user_id);
        assert_eq!(result.event_type, "updated_event");
        assert_eq!(result.event_type_id, 2);
        assert!(result.metadata.is_some());
    }

    #[tokio::test]
    async fn test_update_event_handler_not_found() {
        let (_container, _, update_handler, ..) =
            setup_test_handlers().await.unwrap();

        let non_existent_id = Uuid::now_v7();

        let update_command = UpdateEventCommand {
            event_id: non_existent_id,
            event_type_id: Some(1),
            metadata: Some(json!({"updated": "data"})),
            timestamp: None,
        };

        let result = update_handler.execute(update_command).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_event_handler_partial_update() {
        let (container, create_handler, update_handler, ..) =
            setup_test_handlers().await.unwrap();

        let _event_type_id =
            create_test_event_type(&container).await.unwrap();
        let user_id = create_test_user(&container).await.unwrap();

        // First create an event
        let create_command = CreateEventCommand {
            user_id,
            event_type: "test_event".to_string(),
            timestamp: Some(Utc::now()),
            metadata: Some(json!({"original": "data"})),
        };
        let created_event =
            create_handler.execute(create_command).await.unwrap();

        // Update only metadata, leave other fields unchanged
        let update_command = UpdateEventCommand {
            event_id: created_event.id,
            event_type_id: None, // Don't change
            metadata: Some(json!({"partial": "update"})),
            timestamp: None, // Don't change
        };

        let result = update_handler.execute(update_command).await.unwrap();

        assert_eq!(result.id, created_event.id);
        assert_eq!(result.user_id, user_id);
        assert_eq!(result.event_type, "test_event"); // Should remain unchanged
        assert!(result.metadata.is_some());
    }

    #[tokio::test]
    async fn test_delete_event_handler() {
        let (container, create_handler, _, delete_handler, _) =
            setup_test_handlers().await.unwrap();

        let _event_type_id =
            create_test_event_type(&container).await.unwrap();
        let user_id = create_test_user(&container).await.unwrap();

        // First create an event
        let create_command = CreateEventCommand {
            user_id,
            event_type: "test_event".to_string(),
            timestamp: Some(Utc::now()),
            metadata: Some(json!({"to_delete": "yes"})),
        };
        let created_event =
            create_handler.execute(create_command).await.unwrap();

        // Then delete it
        let delete_command = DeleteEventCommand {
            event_id: created_event.id,
        };

        let result = delete_handler.execute(delete_command).await;
        assert!(result.is_ok());

        // Verify it's actually deleted by trying to find it
        let event_dao = EventDao::new(create_sql_connect(&container));
        let find_result = event_dao.find_by_id(created_event.id).await;
        assert!(find_result.is_err());
    }

    #[tokio::test]
    async fn test_delete_event_handler_not_found() {
        let (_container, _, _, delete_handler, _) =
            setup_test_handlers().await.unwrap();

        let non_existent_id = Uuid::now_v7();

        let delete_command = DeleteEventCommand {
            event_id: non_existent_id,
        };

        let result = delete_handler.execute(delete_command).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_bulk_delete_events_handler() {
        let (container, create_handler, _, _, bulk_delete_handler) =
            setup_test_handlers().await.unwrap();

        let _event_type_id =
            create_test_event_type(&container).await.unwrap();
        let user_id = create_test_user(&container).await.unwrap();

        let now = Utc::now();
        let one_hour_ago = now - Duration::hours(1);
        let two_hours_ago = now - Duration::hours(2);

        // Create events with different timestamps
        for (i, timestamp) in
            [two_hours_ago, one_hour_ago, now].iter().enumerate()
        {
            let create_command = CreateEventCommand {
                user_id,
                event_type: "test_event".to_string(),
                timestamp: Some(*timestamp),
                metadata: Some(json!({"sequence": i})),
            };
            create_handler.execute(create_command).await.unwrap();
        }

        // Delete events before "now" (should delete 2 events)
        let bulk_delete_command = BulkDeleteEventsCommand { before: now };

        let result = bulk_delete_handler
            .execute(bulk_delete_command)
            .await
            .unwrap();

        assert_eq!(result.deleted_count, 2);
        assert_eq!(result.deleted_before, now);
    }

    #[tokio::test]
    async fn test_bulk_delete_events_handler_no_events() {
        let (_container, _, _, _, bulk_delete_handler) =
            setup_test_handlers().await.unwrap();

        let future_time = Utc::now() + Duration::hours(1);

        let bulk_delete_command = BulkDeleteEventsCommand {
            before: future_time,
        };

        let result = bulk_delete_handler
            .execute(bulk_delete_command)
            .await
            .unwrap();

        assert_eq!(result.deleted_count, 0);
        assert_eq!(result.deleted_before, future_time);
    }

    #[tokio::test]
    async fn test_create_event_with_complex_metadata() {
        let (container, create_handler, ..) =
            setup_test_handlers().await.unwrap();

        let _event_type_id =
            create_test_event_type(&container).await.unwrap();
        let user_id = create_test_user(&container).await.unwrap();

        let complex_metadata = json!({
            "action": "purchase",
            "product": {
                "id": 123,
                "name": "Test Product",
                "price": 29.99
            },
            "user_info": {
                "session_id": "abc123",
                "ip_address": "192.168.1.1"
            },
            "tags": ["ecommerce", "conversion"]
        });

        let command = CreateEventCommand {
            user_id,
            event_type: "test_event".to_string(),
            timestamp: Some(Utc::now()),
            metadata: Some(complex_metadata.clone()),
        };

        let result = create_handler.execute(command).await.unwrap();

        assert_eq!(result.user_id, user_id);
        assert!(result.metadata.is_some());
    }
}
