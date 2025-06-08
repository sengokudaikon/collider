use database_traits::dao::GenericDao;
use events_dao::EventDao;
use events_models::CreateEventRequest;
use events_queries::{GetEventError, GetEventQuery, GetEventQueryHandler};
use test_utils::{postgres::TestPostgresContainer, *};
use uuid::Uuid;

async fn setup_test_db(
) -> anyhow::Result<(TestPostgresContainer, GetEventQueryHandler, EventDao)> {
    let container = TestPostgresContainer::new_with_unique_db().await?;

    let sql_connect = create_sql_connect(&container);
    let handler = GetEventQueryHandler::new(sql_connect.clone());
    let dao = EventDao::new(sql_connect);

    Ok((container, handler, dao))
}

#[tokio::test]
async fn test_get_event_success() {
    let (container, handler, dao) = setup_test_db().await.unwrap();
    let event_type_id = create_test_event_type(&container).await.unwrap();
    let user_id = create_test_user(&container).await.unwrap();

    let create_request = CreateEventRequest {
        user_id,
        event_type_id,
        metadata: Some(serde_json::json!({"test": "data"})),
    };
    let created_event = dao.create(create_request).await.unwrap();

    let query = GetEventQuery {
        event_id: created_event.id,
    };
    let result = handler.execute(query).await.unwrap();

    assert_eq!(result.id, created_event.id);
    assert_eq!(result.user_id, user_id);
    assert_eq!(result.event_type_id, event_type_id);
    assert_eq!(result.metadata, Some(serde_json::json!({"test": "data"})));
}

#[tokio::test]
async fn test_get_event_not_found() {
    let (_container, handler, _) = setup_test_db().await.unwrap();
    let non_existent_id = Uuid::now_v7();

    let query = GetEventQuery {
        event_id: non_existent_id,
    };

    let result = handler.execute(query).await;
    assert!(result.is_err());

    match result.unwrap_err() {
        GetEventError::NotFound { event_id } => {
            assert_eq!(event_id, non_existent_id);
        }
        _ => panic!("Expected NotFound error"),
    }
}

#[tokio::test]
async fn test_get_event_with_complex_metadata() {
    let (container, handler, dao) = setup_test_db().await.unwrap();
    let event_type_id = create_test_event_type(&container).await.unwrap();
    let user_id = create_test_user(&container).await.unwrap();

    let complex_metadata = serde_json::json!({
        "user_action": {
            "type": "click",
            "coordinates": {"x": 100, "y": 200},
            "element": "button"
        },
        "session": {
            "id": "sess_123",
            "duration": 1234567,
            "page_views": ["/home", "/products", "/checkout"]
        },
        "device": {
            "type": "mobile",
            "os": "iOS",
            "version": "16.0"
        }
    });

    let create_request = CreateEventRequest {
        user_id,
        event_type_id,
        metadata: Some(complex_metadata.clone()),
    };
    let created_event = dao.create(create_request).await.unwrap();

    let query = GetEventQuery {
        event_id: created_event.id,
    };
    let result = handler.execute(query).await.unwrap();

    assert_eq!(result.metadata, Some(complex_metadata));
}

#[tokio::test]
async fn test_get_event_without_metadata() {
    let (container, handler, dao) = setup_test_db().await.unwrap();
    let event_type_id = create_test_event_type(&container).await.unwrap();
    let user_id = create_test_user(&container).await.unwrap();

    let create_request = CreateEventRequest {
        user_id,
        event_type_id,
        metadata: None,
    };
    let created_event = dao.create(create_request).await.unwrap();

    let query = GetEventQuery {
        event_id: created_event.id,
    };
    let result = handler.execute(query).await.unwrap();

    assert_eq!(result.id, created_event.id);
    assert!(result.metadata.is_none());
}

#[tokio::test]
async fn test_get_multiple_different_events() {
    let (container, handler, dao) = setup_test_db().await.unwrap();
    let event_type_id = create_test_event_type(&container).await.unwrap();
    let user_id = create_test_user(&container).await.unwrap();

    let events = vec![
        serde_json::json!({"action": "login"}),
        serde_json::json!({"action": "view_page", "page": "home"}),
        serde_json::json!({"action": "purchase", "amount": 99.99}),
    ];

    let mut created_event_ids = Vec::new();
    for metadata in events.iter() {
        let create_request = CreateEventRequest {
            user_id,
            event_type_id,
            metadata: Some(metadata.clone()),
        };
        let created_event = dao.create(create_request).await.unwrap();
        created_event_ids.push(created_event.id);
    }

    for (i, event_id) in created_event_ids.iter().enumerate() {
        let query = GetEventQuery {
            event_id: *event_id,
        };
        let result = handler.execute(query).await.unwrap();

        assert_eq!(result.id, *event_id);
        assert_eq!(result.metadata, Some(events[i].clone()));
    }
}
