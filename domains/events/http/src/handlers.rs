use axum::{
    Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post, put},
};
use chrono::{DateTime, Utc};
use domain::AppError;
use events_commands::{
    BulkDeleteEventsCommand, BulkDeleteEventsHandler,
    BulkDeleteEventsResponse, CreateEventCommand, CreateEventHandler,
    CreateEventResponse, DeleteEventHandler, UpdateEventCommand,
    UpdateEventHandler, UpdateEventResponse,
};
use events_queries::{
    GetEventQuery, GetEventQueryHandler, ListEventsQuery,
    ListEventsQueryHandler,
};
use serde::Deserialize;
use sql_connection::SqlConnect;
use tracing::instrument;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::EventResponse;

#[derive(Clone)]
pub struct EventServices {
    pub create_event: CreateEventHandler,
    pub update_event: UpdateEventHandler,
    pub delete_event: DeleteEventHandler,
    pub bulk_delete_events: BulkDeleteEventsHandler,

    pub get_event: GetEventQueryHandler,
    pub list_events: ListEventsQueryHandler,
}

impl EventServices {
    pub fn new(db: SqlConnect) -> Self {
        Self {
            create_event: CreateEventHandler::new(db.clone()),
            update_event: UpdateEventHandler::new(db.clone()),
            delete_event: DeleteEventHandler::new(db.clone()),
            bulk_delete_events: BulkDeleteEventsHandler::new(db.clone()),
            get_event: GetEventQueryHandler::new(db.clone()),
            list_events: ListEventsQueryHandler::new(db),
        }
    }
}

pub struct EventHandlers;

impl EventHandlers {
    pub fn routes() -> Router<EventServices> {
        Router::new()
            .route("/", get(list_events))
            .route("/", post(create_event))
            .route("/", delete(bulk_delete_events))
            .route("/{id}", get(get_event))
            .route("/{id}", put(update_event))
            .route("/{id}", delete(delete_event))
    }
}

#[utoipa::path(
    post,
    path = "/api/events",
    request_body = CreateEventCommand,
    responses(
        (status = 201, description = "Event created successfully", body = CreateEventResponse),
        (status = 400, description = "Invalid request data"),
        (status = 500, description = "Internal server error")
    ),
    tag = "events"
)]
#[instrument(skip_all)]
pub async fn create_event(
    State(services): State<EventServices>,
    Json(command): Json<CreateEventCommand>,
) -> Result<(StatusCode, Json<events_commands::CreateEventResponse>), AppError>
{
    let result = services
        .create_event
        .execute(command)
        .await
        .map_err(AppError::from_error)?;
    Ok((StatusCode::CREATED, Json(result.event)))
}

#[utoipa::path(
    put,
    path = "/api/events/{id}",
    request_body = UpdateEventCommand,
    params(
        ("id" = Uuid, Path, description = "Event ID")
    ),
    responses(
        (status = 200, description = "Event updated successfully", body = UpdateEventResponse),
        (status = 404, description = "Event not found"),
        (status = 400, description = "Invalid request data"),
        (status = 500, description = "Internal server error")
    ),
    tag = "events"
)]
#[instrument(skip_all)]
pub async fn update_event(
    State(services): State<EventServices>, Path(id): Path<Uuid>,
    Json(mut command): Json<UpdateEventCommand>,
) -> Result<Json<events_commands::UpdateEventResponse>, AppError> {
    command.event_id = id;
    let result = services
        .update_event
        .execute(command)
        .await
        .map_err(AppError::from_error)?;
    Ok(Json(result.event))
}

#[utoipa::path(
    delete,
    path = "/api/events/{id}",
    params(
        ("id" = Uuid, Path, description = "Event ID")
    ),
    responses(
        (status = 204, description = "Event deleted successfully"),
        (status = 404, description = "Event not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "events"
)]
#[instrument(skip_all)]
pub async fn delete_event(
    State(services): State<EventServices>, Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let command = events_commands::DeleteEventCommand { event_id: id };
    services
        .delete_event
        .execute(command)
        .await
        .map_err(AppError::from_error)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/events/{id}",
    params(
        ("id" = Uuid, Path, description = "Event ID")
    ),
    responses(
        (status = 200, description = "Event found", body = EventResponse),
        (status = 404, description = "Event not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "events"
)]
#[instrument(skip_all)]
pub async fn get_event(
    State(services): State<EventServices>, Path(id): Path<Uuid>,
) -> Result<Json<EventResponse>, AppError> {
    let query = GetEventQuery { event_id: id };
    let event = services
        .get_event
        .execute(query)
        .await
        .map_err(AppError::from_error)?;
    Ok(Json(event.into()))
}

#[utoipa::path(
    get,
    path = "/api/events",
    params(
        ListEventsParams
    ),
    responses(
        (status = 200, description = "List of events", body = Vec<EventResponse>),
        (status = 400, description = "Invalid query parameters"),
        (status = 500, description = "Internal server error")
    ),
    tag = "events"
)]
#[instrument(skip_all)]
pub async fn list_events(
    State(services): State<EventServices>,
    Query(params): Query<ListEventsParams>,
) -> Result<Json<Vec<EventResponse>>, AppError> {
    let limit = params.limit.unwrap_or(100).min(1000);
    let offset = params
        .offset
        .or_else(|| {
            params.page.map(|p| if p > 0 { (p - 1) * limit } else { 0 })
        })
        .unwrap_or(0);

    let query = ListEventsQuery {
        user_id: params.user_id,
        event_type_id: params.event_type_id,
        limit: Some(limit),
        offset: Some(offset),
    };
    let events = services
        .list_events
        .execute(query)
        .await
        .map_err(AppError::from_error)?;
    Ok(Json(events.into_iter().map(Into::into).collect()))
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct ListEventsParams {
    user_id: Option<Uuid>,
    event_type_id: Option<i32>,
    limit: Option<u64>,
    offset: Option<u64>,
    page: Option<u64>,
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct BulkDeleteParams {
    before: DateTime<Utc>,
}

#[utoipa::path(
    delete,
    path = "/api/events",
    params(
        BulkDeleteParams
    ),
    responses(
        (status = 200, description = "Events deleted successfully", body = BulkDeleteEventsResponse),
        (status = 400, description = "Invalid query parameters"),
        (status = 500, description = "Internal server error")
    ),
    tag = "events"
)]
#[instrument(skip_all)]
pub async fn bulk_delete_events(
    State(services): State<EventServices>,
    Query(params): Query<BulkDeleteParams>,
) -> Result<Json<events_commands::BulkDeleteEventsResponse>, AppError> {
    let command = BulkDeleteEventsCommand {
        before: params.before,
    };
    let result = services
        .bulk_delete_events
        .execute(command)
        .await
        .map_err(AppError::from_error)?;
    Ok(Json(result.result))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Method, Request, StatusCode},
        routing::Router,
    };
    use chrono::Utc;
    use database_traits::dao::GenericDao;
    use events_commands::CreateEventCommand;
    use events_dao::EventDao;
    use events_models::EventActiveModel;
    use sea_orm::ActiveValue::Set;
    use serde_json::json;
    use test_utils::{
        postgres::TestPostgresContainer, redis::TestRedisContainer, *,
    };
    use tower::ServiceExt;
    use uuid::Uuid;

    use super::*;

    async fn setup_test_app()
    -> anyhow::Result<(TestPostgresContainer, Router, EventDao)> {
        let container = TestPostgresContainer::new().await?;

        // Initialize Redis for caching (this also calls
        // RedisConnectionManager::init_static)
        let redis_container = TestRedisContainer::new().await?;
        redis_container.flush_db().await?;

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
        let _event_type_id =
            create_test_event_type(&container).await.unwrap();
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

        let active_model = EventActiveModel {
            id: Set(Uuid::now_v7()),
            user_id: Set(user_id),
            event_type_id: Set(event_type_id),
            timestamp: Set(Utc::now()),
            metadata: Set(Some(json!({"test": "data"}))),
        };
        let created_event = dao.create(active_model).await.unwrap();

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
        let _event_type_id =
            create_test_event_type(&container).await.unwrap();
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
                "INSERT INTO event_types (id, name) VALUES (2, \
                 'updated_event')",
            )
            .await
            .unwrap();

        let active_model = EventActiveModel {
            id: Set(Uuid::now_v7()),
            user_id: Set(user_id),
            event_type_id: Set(event_type_id),
            timestamp: Set(Utc::now()),
            metadata: Set(Some(json!({"original": "data"}))),
        };
        let created_event = dao.create(active_model).await.unwrap();

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

        let create_request = EventActiveModel {
            id: Set(Uuid::now_v7()),
            user_id: Set(user_id),
            event_type_id: Set(event_type_id),
            timestamp: Set(Utc::now()),
            metadata: Set(Some(json!({"to_delete": "yes"}))),
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
            let create_request = EventActiveModel {
                id: Set(Uuid::now_v7()),
                user_id: Set(user_id),
                event_type_id: Set(event_type_id),
                timestamp: Set(Utc::now()),
                metadata: Set(Some(json!({"sequence": i}))),
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
            "INSERT INTO users (id, name, created_at) VALUES ('{}', 'User \
             2', NOW())",
            user_id_2
        );
        container.execute_sql(&query).await.unwrap();

        let create_request_1 = EventActiveModel {
            id: Set(Uuid::now_v7()),
            user_id: Set(user_id_1),
            event_type_id: Set(event_type_id),
            timestamp: Set(Utc::now()),
            metadata: Set(Some(json!({"user": "1"}))),
        };
        let create_request_2 = EventActiveModel {
            id: Set(Uuid::now_v7()),
            user_id: Set(user_id_2),
            event_type_id: Set(event_type_id),
            timestamp: Set(Utc::now()),
            metadata: Set(Some(json!({"user": "2"}))),
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
            let create_request = EventActiveModel {
                id: Set(Uuid::now_v7()),
                user_id: Set(user_id),
                event_type_id: Set(event_type_id),
                timestamp: Set(Utc::now()),
                metadata: Set(Some(json!({"sequence": i}))),
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
}
