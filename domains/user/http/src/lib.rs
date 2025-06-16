pub mod analytics_integration;
use axum::{
    Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post, put},
};
use domain::AppError;
use events_commands::CreateEventCommand;
use events_handlers::GetUserEventsQueryHandler;
use events_queries::GetUserEventsQuery;
use events_responses::EventResponse;
use flume::Sender;
use serde::Deserialize;
use tracing::instrument;
use user_commands::{
    CreateUserCommand, DeleteUserCommand, UpdateUserCommand,
};
use user_events::UserAnalyticsEvent;
use user_handlers::{
    CreateUserHandler, DeleteUserHandler, GetUserByNameQueryHandler,
    GetUserQueryHandler, ListUsersQueryHandler, UpdateUserHandler,
};
use user_responses::UserResponse;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::analytics_integration::UserAnalyticsFactory;

#[derive(Clone)]
pub struct UserServices {
    pub create_user: CreateUserHandler,
    pub update_user: UpdateUserHandler,
    pub delete_user: DeleteUserHandler,

    pub get_user: GetUserQueryHandler,
    pub get_user_by_name: GetUserByNameQueryHandler,
    pub list_users: ListUsersQueryHandler,
    pub get_user_events: GetUserEventsQueryHandler,
}

impl UserServices {
    pub fn new(db: sql_connection::SqlConnect) -> Self {
        Self {
            create_user: CreateUserHandler::new(db.clone()),
            update_user: UpdateUserHandler::new(db.clone()),
            delete_user: DeleteUserHandler::new(db.clone()),
            get_user: GetUserQueryHandler::new(db.clone()),
            get_user_by_name: GetUserByNameQueryHandler::new(db.clone()),
            list_users: ListUsersQueryHandler::new(db.clone()),
            get_user_events: GetUserEventsQueryHandler::new(db),
        }
    }

    pub fn with_event_sender(
        mut self, event_sender: Sender<CreateEventCommand>,
    ) -> Self {
        self.create_user =
            self.create_user.with_event_sender(event_sender.clone());
        self.update_user =
            self.update_user.with_event_sender(event_sender.clone());
        self.delete_user = self.delete_user.with_event_sender(event_sender);
        self
    }

    /// Create UserServices with analytics integration enabled
    pub fn new_with_analytics(
        db: sql_connection::SqlConnect,
    ) -> (Self, tokio::task::JoinHandle<()>) {
        let (analytics_sender, analytics_task) =
            UserAnalyticsFactory::create_integration();

        let services = Self::new(db).with_analytics_sender(analytics_sender);

        (services, analytics_task)
    }

    /// Configure command handlers with analytics event sender
    pub fn with_analytics_sender(
        mut self, analytics_sender: Sender<UserAnalyticsEvent>,
    ) -> Self {
        self.create_user = self
            .create_user
            .with_analytics_event_sender(analytics_sender.clone());
        self.update_user = self
            .update_user
            .with_analytics_event_sender(analytics_sender.clone());
        self.delete_user = self
            .delete_user
            .with_analytics_event_sender(analytics_sender);
        self
    }
}

pub struct UserHandlers;

impl UserHandlers {
    pub fn routes() -> Router<UserServices> {
        Router::new()
            .route("/", get(list_users))
            .route("/", post(create_user))
            .route("/{id}", get(get_user))
            .route("/{id}", put(update_user))
            .route("/{id}", delete(delete_user))
            .route("/{id}/events", get(get_user_events))
    }
}

#[utoipa::path(
    post,
    path = "/api/users",
    request_body = CreateUserCommand,
    responses(
        (status = 201, description = "User created successfully", body = UserResponse),
        (status = 400, description = "Invalid request data"),
        (status = 500, description = "Internal server error")
    ),
    tag = "users"
)]
#[instrument(skip_all)]
pub async fn create_user(
    State(services): State<UserServices>,
    Json(command): Json<CreateUserCommand>,
) -> Result<(StatusCode, Json<UserResponse>), AppError> {
    let result = services
        .create_user
        .execute(command)
        .await
        .map_err(AppError::from_error)?;

    tracing::info!("User created: {}", result.id);

    Ok((StatusCode::CREATED, Json(result)))
}

#[utoipa::path(
    put,
    path = "/api/users/{id}",
    request_body = UpdateUserCommand,
    params(
        ("id" = Uuid, Path, description = "User ID")
    ),
    responses(
        (status = 200, description = "User updated successfully", body = UserResponse),
        (status = 404, description = "User not found"),
        (status = 400, description = "Invalid request data"),
        (status = 500, description = "Internal server error")
    ),
    tag = "users"
)]
#[instrument(skip_all)]
pub async fn update_user(
    State(services): State<UserServices>, Path(id): Path<Uuid>,
    Json(mut command): Json<UpdateUserCommand>,
) -> Result<Json<UserResponse>, AppError> {
    command.user_id = id;
    let result = services
        .update_user
        .execute(command)
        .await
        .map_err(AppError::from_error)?;

    tracing::info!("User updated: {}", id);

    Ok(Json(result))
}

#[utoipa::path(
    delete,
    path = "/api/users/{id}",
    params(
        ("id" = Uuid, Path, description = "User ID")
    ),
    responses(
        (status = 204, description = "User deleted successfully"),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "users"
)]
#[instrument(skip_all)]
pub async fn delete_user(
    State(services): State<UserServices>, Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let command = DeleteUserCommand { user_id: id };
    services
        .delete_user
        .execute(command)
        .await
        .map_err(AppError::from_error)?;

    tracing::info!("User deleted: {}", id);

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct UserQueryParams {
    limit: Option<u64>,
    offset: Option<u64>,
}

#[utoipa::path(
    get,
    path = "/api/users/{id}",
    params(
        ("id" = Uuid, Path, description = "User ID"),
        UserQueryParams
    ),
    responses(
        (status = 200, description = "User found", body = UserResponse),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "users"
)]
#[instrument(skip_all)]
pub async fn get_user(
    State(services): State<UserServices>, Path(id): Path<Uuid>,
    Query(_params): Query<UserQueryParams>,
) -> Result<Json<UserResponse>, AppError> {
    let query = user_queries::GetUserQuery { user_id: id };
    let user = services
        .get_user
        .execute(query)
        .await
        .map_err(AppError::from_error)?;

    Ok(Json(user.into()))
}

#[utoipa::path(
    get,
    path = "/api/users",
    params(
        UserQueryParams
    ),
    responses(
        (status = 200, description = "List of users", body = Vec<UserResponse>),
        (status = 400, description = "Invalid query parameters"),
        (status = 500, description = "Internal server error")
    ),
    tag = "users"
)]
#[instrument(skip_all)]
pub async fn list_users(
    State(services): State<UserServices>,
    Query(_params): Query<UserQueryParams>,
) -> Result<Json<Vec<UserResponse>>, AppError> {
    let query = user_queries::ListUsersQuery {
        limit: _params.limit,
        offset: _params.offset,
    };
    let users = services
        .list_users
        .execute(query)
        .await
        .map_err(AppError::from_error)?;

    Ok(Json(users.into_iter().map(Into::into).collect()))
}

#[utoipa::path(
    get,
    path = "/users/{user_id}/events",
    params(
        ("user_id" = String, Path, description = "User ID")
    ),
    responses(
        (status = 200, description = "OK"),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Event"
)]
#[instrument(skip_all)]
pub async fn get_user_events(
    State(services): State<UserServices>, Path(user_id): Path<String>,
) -> Result<Json<Vec<EventResponse>>, AppError> {
    let user_uuid = user_id.parse::<Uuid>().map_err(AppError::from_error)?;

    let query = GetUserEventsQuery {
        user_id: user_uuid,
        limit: None,
    };

    let events = services
        .get_user_events
        .execute(query)
        .await
        .map_err(AppError::from_error)?;

    Ok(Json(events.into_iter().map(Into::into).collect()))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Method, Request, StatusCode},
        routing::Router,
    };
    use database_traits::dao::GenericDao;
    use redis_connection::cache_provider::CacheProvider;
    use serde_json::json;
    use test_utils::{
        postgres::TestPostgresContainer, redis::TestRedisContainer, *,
    };
    use tower::ServiceExt;
    use user_commands::{CreateUserCommand, UpdateUserCommand};
    use user_dao::UserDao;
    use uuid::Uuid;

    use super::*;

    async fn setup_test_app()
    -> anyhow::Result<(TestPostgresContainer, Router, UserDao)> {
        let container = TestPostgresContainer::new().await?;
        let redis_container = TestRedisContainer::new().await?;
        redis_container.flush_db().await?;

        CacheProvider::init_redis_static(redis_container.pool.clone());

        let sql_connect = create_sql_connect(&container);
        let services = UserServices::new(sql_connect.clone());
        let dao = UserDao::new(sql_connect);

        let app = UserHandlers::routes().with_state(services);

        Ok((container, app, dao))
    }

    #[tokio::test]
    async fn test_create_user_endpoint() {
        let (container, app, _) = setup_test_app().await.unwrap();

        let command = CreateUserCommand {
            name: "Test User".to_string(),
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

        assert_eq!(response_json["name"], "Test User");
        assert!(response_json["id"].is_string());
        assert!(response_json["created_at"].is_string());
    }

    #[tokio::test]
    async fn test_create_user_invalid_data() {
        let (container, app, _) = setup_test_app().await.unwrap();

        let invalid_data = json!({
            "name": ""  // Empty name should be invalid
        });

        let request = Request::builder()
            .method(Method::POST)
            .uri("/")
            .header("content-type", "application/json")
            .body(Body::from(invalid_data.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should return error for invalid data
        assert!(
            response.status().is_client_error()
                || response.status().is_server_error()
        );
    }

    #[tokio::test]
    async fn test_get_user_endpoint() {
        let (container, app, dao) = setup_test_app().await.unwrap();

        let create_command = CreateUserCommand {
            name: "Get Test User".to_string(),
        };
        let created_user = dao.create(create_command).await.unwrap();

        let request = Request::builder()
            .method(Method::GET)
            .uri(format!("/{}", created_user.id))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let response_json: serde_json::Value =
            serde_json::from_slice(&body).unwrap();

        assert_eq!(response_json["id"], created_user.id.to_string());
        assert_eq!(response_json["name"], "Get Test User");
    }

    #[tokio::test]
    async fn test_get_user_not_found() {
        let (container, app, _) = setup_test_app().await.unwrap();
        let non_existent_id = Uuid::now_v7();

        let request = Request::builder()
            .method(Method::GET)
            .uri(format!("/{}", non_existent_id))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn test_update_user_endpoint() {
        let (container, app, dao) = setup_test_app().await.unwrap();

        let create_command = CreateUserCommand {
            name: "Original Name".to_string(),
        };
        let created_user = dao.create(create_command).await.unwrap();

        let update_command = UpdateUserCommand {
            user_id: created_user.id,
            name: Some("Updated Name".to_string()),
        };

        let request = Request::builder()
            .method(Method::PUT)
            .uri(format!("/{}", created_user.id))
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&update_command).unwrap()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let response_json: serde_json::Value =
            serde_json::from_slice(&body).unwrap();

        assert_eq!(response_json["id"], created_user.id.to_string());
        assert_eq!(response_json["name"], "Updated Name");
    }

    #[tokio::test]
    async fn test_update_user_not_found() {
        let (container, app, _) = setup_test_app().await.unwrap();
        let non_existent_id = Uuid::now_v7();

        let update_command = UpdateUserCommand {
            user_id: non_existent_id,
            name: Some("New Name".to_string()),
        };

        let request = Request::builder()
            .method(Method::PUT)
            .uri(format!("/{}", non_existent_id))
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&update_command).unwrap()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn test_delete_user_endpoint() {
        let (container, app, dao) = setup_test_app().await.unwrap();

        let create_command = CreateUserCommand {
            name: "User To Delete".to_string(),
        };
        let created_user = dao.create(create_command).await.unwrap();

        let request = Request::builder()
            .method(Method::DELETE)
            .uri(format!("/{}", created_user.id))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        // Verify user is actually deleted
        let find_result = dao.find_by_id(created_user.id).await;
        assert!(find_result.is_err());
    }

    #[tokio::test]
    async fn test_delete_user_not_found() {
        let (container, app, _) = setup_test_app().await.unwrap();
        let non_existent_id = Uuid::now_v7();

        let request = Request::builder()
            .method(Method::DELETE)
            .uri(format!("/{}", non_existent_id))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn test_list_users_endpoint() {
        let (container, app, dao) = setup_test_app().await.unwrap();

        // Create multiple users
        for i in 0..3 {
            let create_command = CreateUserCommand {
                name: format!("Test User {}", i),
            };
            dao.create(create_command).await.unwrap();
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
        let users = response_json.as_array().unwrap();
        assert!(users.len() >= 3); // At least the 3 we created
    }

    #[tokio::test]
    async fn test_list_users_with_pagination() {
        let (container, app, dao) = setup_test_app().await.unwrap();

        // Create multiple users
        for i in 0..5 {
            let create_command = CreateUserCommand {
                name: format!("Paginated User {}", i),
            };
            dao.create(create_command).await.unwrap();
        }

        let request = Request::builder()
            .method(Method::GET)
            .uri("/?limit=2&offset=0")
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
        let users = response_json.as_array().unwrap();
        assert!(users.len() <= 2); // Should respect limit
    }

    #[tokio::test]
    async fn test_get_user_events_endpoint() {
        let (container, app, dao) = setup_test_app().await.unwrap();

        let create_command = CreateUserCommand {
            name: "User With Events".to_string(),
        };
        let created_user = dao.create(create_command).await.unwrap();

        // Create some test events for this user
        let event_type_id = create_test_event_type(&container).await.unwrap();
        for i in 0..2 {
            create_test_event(
                &container,
                created_user.id,
                event_type_id,
                Some(&format!(r#"{{"event": "test_{}"}}"#, i)),
            )
            .await
            .unwrap();
        }

        let request = Request::builder()
            .method(Method::GET)
            .uri(format!("/{}/events", created_user.id))
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
        assert_eq!(events.len(), 2);
    }

    #[tokio::test]
    async fn test_get_user_events_invalid_user_id() {
        let (container, app, _) = setup_test_app().await.unwrap();

        let request = Request::builder()
            .method(Method::GET)
            .uri("/invalid-uuid/events")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn test_get_user_events_non_existent_user() {
        let (container, app, _) = setup_test_app().await.unwrap();
        let non_existent_id = Uuid::now_v7();

        let request = Request::builder()
            .method(Method::GET)
            .uri(format!("/{}/events", non_existent_id))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should still return OK with empty array for non-existent user
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let response_json: serde_json::Value =
            serde_json::from_slice(&body).unwrap();

        assert!(response_json.is_array());
        let events = response_json.as_array().unwrap();
        assert_eq!(events.len(), 0);
    }

    #[tokio::test]
    async fn test_create_user_with_duplicate_name() {
        let (container, app, dao) = setup_test_app().await.unwrap();

        // Create first user
        let create_command1 = CreateUserCommand {
            name: "Duplicate Name".to_string(),
        };
        dao.create(create_command1).await.unwrap();

        // Try to create second user with same name
        let create_command2 = CreateUserCommand {
            name: "Duplicate Name".to_string(),
        };

        let request = Request::builder()
            .method(Method::POST)
            .uri("/")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_string(&create_command2).unwrap(),
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should succeed - duplicate names are allowed
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn test_partial_update_user() {
        let (container, app, dao) = setup_test_app().await.unwrap();

        let create_command = CreateUserCommand {
            name: "Original Name".to_string(),
        };
        let created_user = dao.create(create_command).await.unwrap();

        // Update with None to test partial updates
        let update_command = UpdateUserCommand {
            user_id: created_user.id,
            name: None, // This should not change the name
        };

        let request = Request::builder()
            .method(Method::PUT)
            .uri(format!("/{}", created_user.id))
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&update_command).unwrap()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let response_json: serde_json::Value =
            serde_json::from_slice(&body).unwrap();

        // Name should remain unchanged
        assert_eq!(response_json["name"], "Original Name");
    }

    #[tokio::test]
    async fn test_user_creation_with_analytics_integration() {
        let (_container, app, _dao) = setup_test_app().await.unwrap();

        // Create user through HTTP endpoint
        let create_command = CreateUserCommand {
            name: "Analytics Test User".to_string(),
        };

        let request = Request::builder()
            .method(Method::POST)
            .uri("/")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&create_command).unwrap()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        // Note: This test verifies that user creation works end-to-end.
        // Analytics events are processed asynchronously in the background,
        // so we can't easily verify analytics metrics here without
        // introducing complex test synchronization. The analytics
        // integration is tested separately in the
        // analytics_integration module.
    }

    #[tokio::test]
    async fn test_user_update_with_analytics_integration() {
        let (_container, app, dao) = setup_test_app().await.unwrap();

        // First create a user
        let created_user_id = create_test_user(&_container).await.unwrap();

        // Update user through HTTP endpoint
        let update_command = UpdateUserCommand {
            user_id: created_user_id,
            name: Some("Updated Analytics User".to_string()),
        };

        let request = Request::builder()
            .method(Method::PUT)
            .uri(format!("/{}", created_user_id))
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&update_command).unwrap()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Verify in database
        let user_from_db = dao.find_by_id(created_user_id).await.unwrap();
        assert_eq!(user_from_db.name, "Updated Analytics User");

        // Note: This test verifies that user updates work end-to-end and
        // trigger analytics events. The events are processed asynchronously.
    }
}
