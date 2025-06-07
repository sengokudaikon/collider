use chrono::Utc;
use events_dao::{EventDao, EventDaoError};
use events_models::{CreateEventRequest, UpdateEventRequest};
use sql_connection::database_traits::dao::GenericDao;
use test_utils::{create_sql_connect, postgres::TestPostgresContainer};
use uuid::Uuid;

async fn setup_test_db() -> anyhow::Result<(TestPostgresContainer, EventDao)>
{
    let container = TestPostgresContainer::new().await?;

    let sql_connect = create_sql_connect(&container);
    let event_dao = EventDao::new(sql_connect);

    Ok((container, event_dao))
}

async fn create_test_event_type(
    container: &TestPostgresContainer,
) -> anyhow::Result<i32> {
    container
        .execute_sql(
            "INSERT INTO event_types (id, name) VALUES (1, 'test_event')",
        )
        .await?;
    Ok(1)
}

async fn create_test_user(
    container: &TestPostgresContainer,
) -> anyhow::Result<Uuid> {
    let user_id = Uuid::now_v7();
    let query = format!(
        "INSERT INTO users (id, name, email, created_at, updated_at) VALUES \
         ('{}', 'Test User', 'test@example.com', NOW(), NOW())",
        user_id
    );
    container.execute_sql(&query).await?;
    Ok(user_id)
}

#[tokio::test]
async fn test_create_event_success() {
    let (container, event_dao) = setup_test_db().await.unwrap();
    let event_type_id = create_test_event_type(&container).await.unwrap();
    let user_id = create_test_user(&container).await.unwrap();

    let create_request = CreateEventRequest {
        user_id,
        event_type_id,
        metadata: Some(serde_json::json!({"key": "value"})),
    };

    let result = event_dao.create(create_request).await.unwrap();

    assert_eq!(result.user_id, user_id);
    assert_eq!(result.event_type_id, event_type_id);
    assert!(result.metadata.is_some());
    assert!(result.timestamp <= Utc::now());
}

#[tokio::test]
async fn test_find_by_id_success() {
    let (container, event_dao) = setup_test_db().await.unwrap();
    let event_type_id = create_test_event_type(&container).await.unwrap();
    let user_id = create_test_user(&container).await.unwrap();

    let create_request = CreateEventRequest {
        user_id,
        event_type_id,
        metadata: Some(serde_json::json!({"test": "data"})),
    };

    let created_event = event_dao.create(create_request).await.unwrap();
    let found_event = event_dao.find_by_id(created_event.id).await.unwrap();

    assert_eq!(found_event.id, created_event.id);
    assert_eq!(found_event.user_id, user_id);
    assert_eq!(found_event.event_type_id, event_type_id);
    assert_eq!(found_event.metadata, created_event.metadata);
}

#[tokio::test]
async fn test_find_by_id_not_found() {
    let (_container, event_dao) = setup_test_db().await.unwrap();
    let non_existent_id = Uuid::now_v7();

    let result = event_dao.find_by_id(non_existent_id).await;

    assert!(matches!(result, Err(EventDaoError::NotFound)));
}

#[tokio::test]
async fn test_update_event_success() {
    let (container, event_dao) = setup_test_db().await.unwrap();
    let event_type_id = create_test_event_type(&container).await.unwrap();
    let user_id = create_test_user(&container).await.unwrap();

    // Create another event type for update test
    container
        .execute_sql(
            "INSERT INTO event_types (id, name, description) VALUES (2, \
             'updated_event', 'Updated event type')",
        )
        .await
        .unwrap();

    let create_request = CreateEventRequest {
        user_id,
        event_type_id,
        metadata: Some(serde_json::json!({"original": "data"})),
    };

    let created_event = event_dao.create(create_request).await.unwrap();

    let update_request = UpdateEventRequest {
        event_type_id: Some(2),
        metadata: Some(serde_json::json!({"updated": "data"})),
    };

    let updated_event = event_dao
        .update(created_event.id, update_request)
        .await
        .unwrap();

    assert_eq!(updated_event.id, created_event.id);
    assert_eq!(updated_event.event_type_id, 2);
    assert_eq!(
        updated_event.metadata,
        Some(serde_json::json!({"updated": "data"}))
    );
}

#[tokio::test]
async fn test_delete_event_success() {
    let (container, event_dao) = setup_test_db().await.unwrap();
    let event_type_id = create_test_event_type(&container).await.unwrap();
    let user_id = create_test_user(&container).await.unwrap();

    let create_request = CreateEventRequest {
        user_id,
        event_type_id,
        metadata: None,
    };

    let created_event = event_dao.create(create_request).await.unwrap();

    // Delete the event
    event_dao.delete(created_event.id).await.unwrap();

    // Verify it's deleted
    let result = event_dao.find_by_id(created_event.id).await;
    assert!(matches!(result, Err(EventDaoError::NotFound)));
}

#[tokio::test]
async fn test_find_with_filters_by_user() {
    let (container, event_dao) = setup_test_db().await.unwrap();
    let event_type_id = create_test_event_type(&container).await.unwrap();
    let user_id_1 = create_test_user(&container).await.unwrap();

    // Create second user
    let user_id_2 = Uuid::now_v7();
    let query = format!(
        "INSERT INTO users (id, name, email, created_at, updated_at) VALUES \
         ('{}', 'Test User 2', 'test2@example.com', NOW(), NOW())",
        user_id_2
    );
    container.execute_sql(&query).await.unwrap();

    // Create events for both users
    for user_id in [user_id_1, user_id_2] {
        let create_request = CreateEventRequest {
            user_id,
            event_type_id,
            metadata: Some(
                serde_json::json!({"user_id": user_id.to_string()}),
            ),
        };
        event_dao.create(create_request).await.unwrap();
    }

    // Filter by user_id_1
    let events = event_dao
        .find_with_filters(Some(user_id_1), None, None, None)
        .await
        .unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].user_id, user_id_1);
}

#[tokio::test]
async fn test_find_with_filters_by_event_type() {
    let (container, event_dao) = setup_test_db().await.unwrap();
    let event_type_id_1 = create_test_event_type(&container).await.unwrap();

    // Create second event type
    container
        .execute_sql(
            "INSERT INTO event_types (id, name, description) VALUES (2, \
             'other_event', 'Other event type')",
        )
        .await
        .unwrap();

    let user_id = create_test_user(&container).await.unwrap();

    // Create events for both event types
    for event_type_id in [event_type_id_1, 2] {
        let create_request = CreateEventRequest {
            user_id,
            event_type_id,
            metadata: Some(
                serde_json::json!({"event_type_id": event_type_id}),
            ),
        };
        event_dao.create(create_request).await.unwrap();
    }

    // Filter by event_type_id_1
    let events = event_dao
        .find_with_filters(None, Some(event_type_id_1), None, None)
        .await
        .unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type_id, event_type_id_1);
}

#[tokio::test]
async fn test_find_with_pagination() {
    let (container, event_dao) = setup_test_db().await.unwrap();
    let event_type_id = create_test_event_type(&container).await.unwrap();
    let user_id = create_test_user(&container).await.unwrap();

    // Create 5 events
    for i in 0..5 {
        let create_request = CreateEventRequest {
            user_id,
            event_type_id,
            metadata: Some(serde_json::json!({"sequence": i})),
        };
        event_dao.create(create_request).await.unwrap();
    }

    // Test limit
    let events = event_dao
        .find_with_filters(None, None, Some(2), None)
        .await
        .unwrap();
    assert_eq!(events.len(), 2);

    // Test offset
    let events = event_dao
        .find_with_filters(None, None, Some(2), Some(2))
        .await
        .unwrap();
    assert_eq!(events.len(), 2);

    // Test limit + offset
    let events = event_dao
        .find_with_filters(None, None, Some(1), Some(4))
        .await
        .unwrap();
    assert_eq!(events.len(), 1);
}

#[tokio::test]
async fn test_all_events() {
    let (container, event_dao) = setup_test_db().await.unwrap();
    let event_type_id = create_test_event_type(&container).await.unwrap();
    let user_id = create_test_user(&container).await.unwrap();

    // Create 3 events
    for i in 0..3 {
        let create_request = CreateEventRequest {
            user_id,
            event_type_id,
            metadata: Some(serde_json::json!({"index": i})),
        };
        event_dao.create(create_request).await.unwrap();
    }

    let all_events = event_dao.all().await.unwrap();
    assert_eq!(all_events.len(), 3);
}
