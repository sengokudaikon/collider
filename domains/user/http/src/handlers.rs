use axum::{
    Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post, put},
};
use crate::analytics_integration::UserAnalyticsFactory;
use domain::AppError;
use events_commands::CreateEventCommand;
use flume::Sender;
use serde::Deserialize;
use tracing::instrument;
use user_commands::{
    CreateUserCommand, CreateUserResponse, DeleteUserCommand, 
    UpdateUserCommand, UpdateUserResponse,
};
use crate::command_handlers::{
    CreateUserHandler, UpdateUserHandler, DeleteUserHandler,
};
use user_events::UserAnalyticsEvent;
use user_queries::{
    GetUserByNameQueryHandler, GetUserQueryHandler, ListUsersQueryHandler,
};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::UserResponse;

#[derive(Clone)]
pub struct UserServices {
    pub create_user: CreateUserHandler,
    pub update_user: UpdateUserHandler,
    pub delete_user: DeleteUserHandler,

    pub get_user: GetUserQueryHandler,
    pub get_user_by_name: GetUserByNameQueryHandler,
    pub list_users: ListUsersQueryHandler,
}

impl UserServices {
    pub fn new(db: sql_connection::SqlConnect) -> Self {
        Self {
            create_user: CreateUserHandler::new(db.clone()),
            update_user: UpdateUserHandler::new(db.clone()),
            delete_user: DeleteUserHandler::new(db.clone()),
            get_user: GetUserQueryHandler::new(db.clone()),
            get_user_by_name: GetUserByNameQueryHandler::new(db.clone()),
            list_users: ListUsersQueryHandler::new(db),
        }
    }

    pub fn with_event_sender(mut self, event_sender: Sender<CreateEventCommand>) -> Self {
        self.create_user = self.create_user.with_event_sender(event_sender.clone());
        self.update_user = self.update_user.with_event_sender(event_sender.clone());
        self.delete_user = self.delete_user.with_event_sender(event_sender);
        self
    }

    /// Create UserServices with analytics integration enabled
    pub fn new_with_analytics(db: sql_connection::SqlConnect) -> (Self, tokio::task::JoinHandle<()>) {
        let (analytics_sender, analytics_task) = UserAnalyticsFactory::create_integration();
        
        let services = Self::new(db).with_analytics_sender(analytics_sender);
        
        (services, analytics_task)
    }

    /// Configure command handlers with analytics event sender
    pub fn with_analytics_sender(mut self, analytics_sender: Sender<UserAnalyticsEvent>) -> Self {
        self.create_user = self.create_user.with_analytics_event_sender(analytics_sender.clone());
        self.update_user = self.update_user.with_analytics_event_sender(analytics_sender.clone());
        self.delete_user = self.delete_user.with_analytics_event_sender(analytics_sender);
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
            .route("/by-name/{username}", get(get_user_by_name))
            .route("/{id}/metrics", get(get_user_with_metrics))
    }
}

#[utoipa::path(
    post,
    path = "/api/users",
    request_body = CreateUserCommand,
    responses(
        (status = 201, description = "User created successfully", body = CreateUserResponse),
        (status = 400, description = "Invalid request data"),
        (status = 500, description = "Internal server error")
    ),
    tag = "users"
)]
#[instrument(skip_all)]
pub async fn create_user(
    State(services): State<UserServices>,
    Json(command): Json<CreateUserCommand>,
) -> Result<(StatusCode, Json<user_commands::CreateUserResponse>), AppError> {
    let result = services
        .create_user
        .execute(command)
        .await
        .map_err(AppError::from_error)?;

    tracing::info!("User created: {}", result.user.id);

    Ok((StatusCode::CREATED, Json(result.user)))
}

#[utoipa::path(
    put,
    path = "/api/users/{id}",
    request_body = UpdateUserCommand,
    params(
        ("id" = Uuid, Path, description = "User ID")
    ),
    responses(
        (status = 200, description = "User updated successfully", body = UpdateUserResponse),
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
) -> Result<Json<user_commands::UpdateUserResponse>, AppError> {
    command.user_id = id;
    let result = services
        .update_user
        .execute(command)
        .await
        .map_err(AppError::from_error)?;

    tracing::info!("User updated: {}", id);

    Ok(Json(result.user))
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
    #[serde(default)]
    include_metrics: bool,
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

    // Analytics functionality removed - using pure user domain now
    // Metrics are handled by the analytics domain via background events
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

    // Analytics functionality removed - using pure user domain now
    // Metrics are handled by the analytics domain via background events
    Ok(Json(users.into_iter().map(Into::into).collect()))
}

#[utoipa::path(
    get,
    path = "/api/users/by-name/{username}",
    params(
        ("username" = String, Path, description = "Username")
    ),
    responses(
        (status = 200, description = "User found", body = UserResponse),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "users"
)]
#[instrument(skip_all)]
pub async fn get_user_by_name(
    State(services): State<UserServices>, Path(name): Path<String>,
) -> Result<Json<UserResponse>, AppError> {
    let query = user_queries::GetUserByNameQuery { name };
    let user = services
        .get_user_by_name
        .execute(query)
        .await
        .map_err(AppError::from_error)?;
    Ok(Json(user.into()))
}

#[utoipa::path(
    get,
    path = "/api/users/{id}/metrics",
    params(
        ("id" = Uuid, Path, description = "User ID")
    ),
    responses(
        (status = 200, description = "User with metrics", body = UserResponse),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "users"
)]
#[instrument(skip_all)]
pub async fn get_user_with_metrics(
    State(services): State<UserServices>, Path(id): Path<Uuid>,
) -> Result<Json<UserResponse>, AppError> {
    let user_query = user_queries::GetUserQuery { user_id: id };
    let user = services
        .get_user
        .execute(user_query)
        .await
        .map_err(AppError::from_error)?;

    // Analytics functionality removed - using pure user domain now
    // Metrics are handled by the analytics domain via background events
    Ok(Json(user.into()))
}
