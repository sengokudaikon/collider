use chrono::Utc;
use database_traits::dao::GenericDao;
use events_commands::{UpdateEventCommand, UpdateEventHandler};
use events_dao::EventDao;
use events_models::EventActiveModel;
use sea_orm::ActiveValue::Set;
use test_utils::{postgres::TestPostgresContainer, *};
use uuid::Uuid;

async fn setup_test_db()
-> anyhow::Result<(TestPostgresContainer, UpdateEventHandler, EventDao)> {
    let container = TestPostgresContainer::new_with_unique_db().await?;

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
    let (_, new_type) = create_test_event_types(&container).await.unwrap();
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
