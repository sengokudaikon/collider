use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
    routing::Router,
};
use database_traits::dao::GenericDao;
use events_commands::CreateEventCommand;
use events_dao::EventDao;
use events_http::{EventHandlers, EventServices};
use events_models::CreateEventRequest;
use serde_json::json;
use test_utils::{postgres::TestPostgresContainer, *};
use tower::ServiceExt;
use uuid::Uuid;

async fn setup_test_app()
-> anyhow::Result<(TestPostgresContainer, Router, EventDao)> {
    let container = TestPostgresContainer::new().await?;

    let sql_connect = create_sql_connect(&container);
    let services = EventServices::new(sql_connect.clone());
    let dao = EventDao::new(sql_connect);

    let app = EventHandlers::routes().with_state(services);

    Ok((container, app, dao))
}

#[tokio::test]
async fn test_create_event_endpoint() {
    let (container, app, _) = setup_test_app().await.unwrap();
    let event_type_id = create_test_event_type(&container).await.unwrap();
    let user_id = create_test_user(&container).await.unwrap();

    let command = CreateEventCommand {
        user_id,
        event_type: "test_event".to_string(),
        timestamp: None,
        metadata: Some(json!({"action": "click", "button": "submit"})),
    };

    let request = Request::builder()
        .method(Method::POST)
        .uri("/")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&command).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: serde_json::Value =
        serde_json::from_slice(&body).unwrap();

    assert_eq!(response_json["user_id"], user_id.to_string());
    assert_eq!(response_json["event_type_id"], event_type_id);
    assert_eq!(response_json["metadata"]["action"], "click");
}

#[tokio::test]
async fn test_create_event_invalid_data() {
    let (container, app, _) = setup_test_app().await.unwrap();
    let _event_type_id = create_test_event_type(&container).await.unwrap();
    let user_id = create_test_user(&container).await.unwrap();

    let command = CreateEventCommand {
        user_id,
        event_type: "non_existent_event_type".to_string(),
        timestamp: None,
        metadata: None,
    };

    let request = Request::builder()
        .method(Method::POST)
        .uri("/")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&command).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_get_event_endpoint() {
    let (container, app, dao) = setup_test_app().await.unwrap();
    let event_type_id = create_test_event_type(&container).await.unwrap();
    let user_id = create_test_user(&container).await.unwrap();

    let create_request = CreateEventRequest {
        user_id,
        event_type_id,
        metadata: Some(json!({"test": "data"})),
    };
    let created_event = dao.create(create_request).await.unwrap();

    let request = Request::builder()
        .method(Method::GET)
        .uri(&format!("/{}", created_event.id))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: serde_json::Value =
        serde_json::from_slice(&body).unwrap();

    assert_eq!(response_json["id"], created_event.id.to_string());
    assert_eq!(response_json["user_id"], user_id.to_string());
    assert_eq!(response_json["event_type_id"], event_type_id);
}

#[tokio::test]
async fn test_get_event_not_found() {
    let (container, app, _) = setup_test_app().await.unwrap();
    let _event_type_id = create_test_event_type(&container).await.unwrap();
    let non_existent_id = Uuid::now_v7();

    let request = Request::builder()
        .method(Method::GET)
        .uri(&format!("/{}", non_existent_id))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_update_event_endpoint() {
    let (container, app, dao) = setup_test_app().await.unwrap();
    let event_type_id = create_test_event_type(&container).await.unwrap();
    let user_id = create_test_user(&container).await.unwrap();

    container
        .execute_sql(
            "INSERT INTO event_types (id, name) VALUES (2, 'updated_event')",
        )
        .await
        .unwrap();

    let create_request = CreateEventRequest {
        user_id,
        event_type_id,
        metadata: Some(json!({"original": "data"})),
    };
    let created_event = dao.create(create_request).await.unwrap();

    let update_data = json!({
        "event_type_id": 2,
        "metadata": {"updated": "metadata"}
    });

    let request = Request::builder()
        .method(Method::PUT)
        .uri(&format!("/{}", created_event.id))
        .header("content-type", "application/json")
        .body(Body::from(update_data.to_string()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: serde_json::Value =
        serde_json::from_slice(&body).unwrap();

    assert_eq!(response_json["id"], created_event.id.to_string());
    assert_eq!(response_json["event_type_id"], 2);
    assert_eq!(response_json["metadata"]["updated"], "metadata");
}

#[tokio::test]
async fn test_delete_event_endpoint() {
    let (container, app, dao) = setup_test_app().await.unwrap();
    let event_type_id = create_test_event_type(&container).await.unwrap();
    let user_id = create_test_user(&container).await.unwrap();

    let create_request = CreateEventRequest {
        user_id,
        event_type_id,
        metadata: Some(json!({"to_delete": "yes"})),
    };
    let created_event = dao.create(create_request).await.unwrap();

    let request = Request::builder()
        .method(Method::DELETE)
        .uri(&format!("/{}", created_event.id))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let find_result = dao.find_by_id(created_event.id).await;
    assert!(find_result.is_err());
}

#[tokio::test]
async fn test_list_events_endpoint() {
    let (container, app, dao) = setup_test_app().await.unwrap();
    let event_type_id = create_test_event_type(&container).await.unwrap();
    let user_id = create_test_user(&container).await.unwrap();

    for i in 0..3 {
        let create_request = CreateEventRequest {
            user_id,
            event_type_id,
            metadata: Some(json!({"sequence": i})),
        };
        dao.create(create_request).await.unwrap();
    }

    let request = Request::builder()
        .method(Method::GET)
        .uri("/")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: serde_json::Value =
        serde_json::from_slice(&body).unwrap();

    assert!(response_json.is_array());
    assert_eq!(response_json.as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn test_list_events_with_filters() {
    let (container, app, dao) = setup_test_app().await.unwrap();
    let event_type_id = create_test_event_type(&container).await.unwrap();
    let user_id_1 = create_test_user(&container).await.unwrap();

    let user_id_2 = Uuid::now_v7();
    let query = format!(
        "INSERT INTO users (id, name, created_at) VALUES ('{}', 'User 2', \
         NOW())",
        user_id_2
    );
    container.execute_sql(&query).await.unwrap();

    let create_request_1 = CreateEventRequest {
        user_id: user_id_1,
        event_type_id,
        metadata: Some(json!({"user": "1"})),
    };
    let create_request_2 = CreateEventRequest {
        user_id: user_id_2,
        event_type_id,
        metadata: Some(json!({"user": "2"})),
    };
    dao.create(create_request_1).await.unwrap();
    dao.create(create_request_2).await.unwrap();

    let request = Request::builder()
        .method(Method::GET)
        .uri(&format!("/?user_id={}", user_id_1))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: serde_json::Value =
        serde_json::from_slice(&body).unwrap();

    assert!(response_json.is_array());
    let events = response_json.as_array().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["user_id"], user_id_1.to_string());
}

#[tokio::test]
async fn test_list_events_with_pagination() {
    let (container, app, dao) = setup_test_app().await.unwrap();
    let event_type_id = create_test_event_type(&container).await.unwrap();
    let user_id = create_test_user(&container).await.unwrap();

    for i in 0..5 {
        let create_request = CreateEventRequest {
            user_id,
            event_type_id,
            metadata: Some(json!({"sequence": i})),
        };
        dao.create(create_request).await.unwrap();
    }

    let request = Request::builder()
        .method(Method::GET)
        .uri("/?limit=2")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: serde_json::Value =
        serde_json::from_slice(&body).unwrap();

    assert!(response_json.is_array());
    assert_eq!(response_json.as_array().unwrap().len(), 2);
}
