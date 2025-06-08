use chrono::Utc;
use events_dao::{EventDao, EventDaoError};
use events_models::{CreateEventRequest, UpdateEventRequest};
use sql_connection::database_traits::dao::GenericDao;
use test_utils::{
    clean_test_data, create_sql_connect, create_test_event_type,
    create_test_event_type_with_name, create_test_user,
    create_test_user_with_name, postgres::TestPostgresContainer,
};
use uuid::Uuid;

async fn setup_test_db() -> anyhow::Result<(TestPostgresContainer, EventDao)>
{
    let container = TestPostgresContainer::new().await?;

    // Clean any existing test data to ensure test isolation
    let _ = clean_test_data(&container).await;

    let sql_connect = create_sql_connect(&container);
    let event_dao = EventDao::new(sql_connect);

    Ok((container, event_dao))
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

    let updated_event_type_id =
        create_test_event_type_with_name(&container, "updated_event")
            .await
            .unwrap();

    let create_request = CreateEventRequest {
        user_id,
        event_type_id,
        metadata: Some(serde_json::json!({"original": "data"})),
    };

    let created_event = event_dao.create(create_request).await.unwrap();

    let update_request = UpdateEventRequest {
        event_type_id: Some(updated_event_type_id),
        metadata: Some(serde_json::json!({"updated": "data"})),
    };

    let updated_event = event_dao
        .update(created_event.id, update_request)
        .await
        .unwrap();

    assert_eq!(updated_event.id, created_event.id);
    assert_eq!(updated_event.event_type_id, updated_event_type_id);
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

    event_dao.delete(created_event.id).await.unwrap();

    let result = event_dao.find_by_id(created_event.id).await;
    assert!(matches!(result, Err(EventDaoError::NotFound)));
}

#[tokio::test]
async fn test_find_with_filters_by_user() {
    let (container, event_dao) = setup_test_db().await.unwrap();
    let event_type_id = create_test_event_type(&container).await.unwrap();
    let user_id_1 = create_test_user(&container).await.unwrap();

    let user_id_2 = create_test_user_with_name(&container, "Test User 2")
        .await
        .unwrap();

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

    let event_type_id_2 =
        create_test_event_type_with_name(&container, "other_event")
            .await
            .unwrap();

    let user_id = create_test_user(&container).await.unwrap();

    for event_type_id in [event_type_id_1, event_type_id_2] {
        let create_request = CreateEventRequest {
            user_id,
            event_type_id,
            metadata: Some(
                serde_json::json!({"event_type_id": event_type_id}),
            ),
        };
        event_dao.create(create_request).await.unwrap();
    }

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

    for i in 0..5 {
        let create_request = CreateEventRequest {
            user_id,
            event_type_id,
            metadata: Some(serde_json::json!({"sequence": i})),
        };
        event_dao.create(create_request).await.unwrap();
    }

    let events = event_dao
        .find_with_filters(None, None, Some(2), None)
        .await
        .unwrap();
    assert_eq!(events.len(), 2);

    let events = event_dao
        .find_with_filters(None, None, Some(2), Some(2))
        .await
        .unwrap();
    assert_eq!(events.len(), 2);

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
