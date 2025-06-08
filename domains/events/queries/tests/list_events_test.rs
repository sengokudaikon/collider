use database_traits::dao::GenericDao;
use events_dao::EventDao;
use events_models::CreateEventRequest;
use events_queries::{ListEventsQuery, ListEventsQueryHandler};
use test_utils::{postgres::TestPostgresContainer, *};

async fn setup_test_db(
) -> anyhow::Result<(TestPostgresContainer, ListEventsQueryHandler, EventDao)>
{
    let container = TestPostgresContainer::new_with_unique_db().await?;

    let sql_connect = create_sql_connect(&container);
    let handler = ListEventsQueryHandler::new(sql_connect.clone());
    let dao = EventDao::new(sql_connect);

    Ok((container, handler, dao))
}

#[tokio::test]
async fn test_list_all_events() {
    let (container, handler, dao) = setup_test_db().await.unwrap();
    let (login_type, purchase_type) =
        create_test_event_types(&container).await.unwrap();
    let (user_1, user_2) = create_test_users(&container).await.unwrap();

    let events_data = vec![
        (user_1, login_type, serde_json::json!({"ip": "192.168.1.1"})),
        (user_1, purchase_type, serde_json::json!({"amount": 50.0})),
        (user_2, login_type, serde_json::json!({"ip": "192.168.1.2"})),
        (user_2, purchase_type, serde_json::json!({"amount": 100.0})),
    ];

    for (user_id, event_type_id, metadata) in events_data {
        let create_request = CreateEventRequest {
            user_id,
            event_type_id,
            metadata: Some(metadata),
        };
        dao.create(create_request).await.unwrap();
    }

    let query = ListEventsQuery {
        user_id: None,
        event_type_id: None,
        limit: None,
        offset: None,
    };
    let result = handler.execute(query).await.unwrap();

    assert_eq!(result.len(), 4);
}

#[tokio::test]
async fn test_list_events_by_user() {
    let (container, handler, dao) = setup_test_db().await.unwrap();
    let (login_type, purchase_type) =
        create_test_event_types(&container).await.unwrap();
    let (user_1, user_2) = create_test_users(&container).await.unwrap();

    let create_request_1 = CreateEventRequest {
        user_id: user_1,
        event_type_id: login_type,
        metadata: Some(serde_json::json!({"user": "1"})),
    };
    let create_request_2 = CreateEventRequest {
        user_id: user_2,
        event_type_id: purchase_type,
        metadata: Some(serde_json::json!({"user": "2"})),
    };
    let create_request_3 = CreateEventRequest {
        user_id: user_1,
        event_type_id: purchase_type,
        metadata: Some(serde_json::json!({"user": "1_again"})),
    };

    dao.create(create_request_1).await.unwrap();
    dao.create(create_request_2).await.unwrap();
    dao.create(create_request_3).await.unwrap();

    let query = ListEventsQuery {
        user_id: Some(user_1),
        event_type_id: None,
        limit: None,
        offset: None,
    };
    let result = handler.execute(query).await.unwrap();

    assert_eq!(result.len(), 2);
    for event in result {
        assert_eq!(event.user_id, user_1);
    }
}

#[tokio::test]
async fn test_list_events_by_event_type() {
    let (container, handler, dao) = setup_test_db().await.unwrap();
    clean_test_data(&container).await.unwrap();
    let (login_type, purchase_type) =
        create_test_event_types(&container).await.unwrap();
    let (user_1, user_2) = create_test_users(&container).await.unwrap();

    let events = vec![
        (user_1, login_type, "login_1"),
        (user_2, login_type, "login_2"),
        (user_1, purchase_type, "purchase_1"),
        (user_2, purchase_type, "purchase_2"),
        (user_1, login_type, "login_3"),
    ];

    for (user_id, event_type_id, tag) in events {
        let create_request = CreateEventRequest {
            user_id,
            event_type_id,
            metadata: Some(serde_json::json!({"tag": tag})),
        };
        dao.create(create_request).await.unwrap();
    }

    let query = ListEventsQuery {
        user_id: None,
        event_type_id: Some(login_type),
        limit: None,
        offset: None,
    };
    let result = handler.execute(query).await.unwrap();

    assert_eq!(result.len(), 3);
    for event in result {
        assert_eq!(event.event_type_id, login_type);
    }
}

#[tokio::test]
async fn test_list_events_with_pagination() {
    let (container, handler, dao) = setup_test_db().await.unwrap();
    let (login_type, _) = create_test_event_types(&container).await.unwrap();
    let (user_1, _) = create_test_users(&container).await.unwrap();

    for i in 0..10 {
        let create_request = CreateEventRequest {
            user_id: user_1,
            event_type_id: login_type,
            metadata: Some(serde_json::json!({"sequence": i})),
        };
        dao.create(create_request).await.unwrap();
    }

    let query = ListEventsQuery {
        user_id: None,
        event_type_id: None,
        limit: Some(5),
        offset: None,
    };
    let result = handler.execute(query).await.unwrap();
    assert_eq!(result.len(), 5);

    let query = ListEventsQuery {
        user_id: None,
        event_type_id: None,
        limit: Some(3),
        offset: Some(7),
    };
    let result = handler.execute(query).await.unwrap();
    assert_eq!(result.len(), 3);

    let query = ListEventsQuery {
        user_id: None,
        event_type_id: None,
        limit: Some(5),
        offset: Some(8),
    };
    let result = handler.execute(query).await.unwrap();
    assert_eq!(result.len(), 2);
}

#[tokio::test]
async fn test_list_events_with_combined_filters() {
    let (container, handler, dao) = setup_test_db().await.unwrap();
    let (login_type, purchase_type) =
        create_test_event_types(&container).await.unwrap();
    let (user_1, user_2) = create_test_users(&container).await.unwrap();

    let events = vec![
        (user_1, login_type, "user1_login"),
        (user_1, purchase_type, "user1_purchase"),
        (user_2, login_type, "user2_login"),
        (user_2, purchase_type, "user2_purchase"),
        (user_1, login_type, "user1_login2"),
    ];

    for (user_id, event_type_id, tag) in events {
        let create_request = CreateEventRequest {
            user_id,
            event_type_id,
            metadata: Some(serde_json::json!({"tag": tag})),
        };
        dao.create(create_request).await.unwrap();
    }

    let query = ListEventsQuery {
        user_id: Some(user_1),
        event_type_id: Some(login_type),
        limit: None,
        offset: None,
    };
    let result = handler.execute(query).await.unwrap();

    assert_eq!(result.len(), 2);
    for event in result {
        assert_eq!(event.user_id, user_1);
        assert_eq!(event.event_type_id, login_type);
    }
}

#[tokio::test]
async fn test_list_events_empty_result() {
    let (container, handler, _) = setup_test_db().await.unwrap();
    let (..) = create_test_event_types(&container).await.unwrap();
    let (user_1, _) = create_test_users(&container).await.unwrap();

    let query = ListEventsQuery {
        user_id: Some(user_1),
        event_type_id: None,
        limit: None,
        offset: None,
    };
    let result = handler.execute(query).await.unwrap();

    assert_eq!(result.len(), 0);
}

#[tokio::test]
async fn test_list_events_edge_cases() {
    let (container, handler, dao) = setup_test_db().await.unwrap();
    let (login_type, _) = create_test_event_types(&container).await.unwrap();
    let (user_1, _) = create_test_users(&container).await.unwrap();

    let create_request = CreateEventRequest {
        user_id: user_1,
        event_type_id: login_type,
        metadata: Some(serde_json::json!({"test": "single"})),
    };
    dao.create(create_request).await.unwrap();

    let query = ListEventsQuery {
        user_id: None,
        event_type_id: None,
        limit: Some(0),
        offset: None,
    };
    let result = handler.execute(query).await.unwrap();
    assert_eq!(result.len(), 0);

    let query = ListEventsQuery {
        user_id: None,
        event_type_id: None,
        limit: Some(1000),
        offset: None,
    };
    let result = handler.execute(query).await.unwrap();
    assert_eq!(result.len(), 1);
}
