use chrono::Utc;
use events_commands::{CreateEventCommand, CreateEventHandler};
use test_utils::{postgres::TestPostgresContainer, *};
use uuid::Uuid;

async fn setup_test_db()
-> anyhow::Result<(TestPostgresContainer, CreateEventHandler)> {
    let container = TestPostgresContainer::new_with_unique_db().await?;

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
