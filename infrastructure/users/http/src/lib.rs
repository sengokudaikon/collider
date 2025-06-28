use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use common_errors::AppError;
use events_queries::GetUserEventsQuery;
use events_query_handlers::GetUserEventsQueryHandler;
use events_responses::EventResponse;
use serde::Deserialize;
use tracing::instrument;
use user_command_handlers::{
    CreateUserHandler, DeleteUserHandler, UpdateUserHandler,
};
use user_commands::{
    CreateUserCommand, DeleteUserCommand, UpdateUserCommand,
};
use user_query_handlers::{
    GetUserByNameQueryHandler, GetUserQueryHandler, ListUsersQueryHandler,
};
use user_responses::UserResponse;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

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
}

#[utoipa::path(
    post,
    path = "/user",
    request_body = CreateUserCommand,
    responses(
        (status = 201, description = "User created successfully", body = UserResponse),
        (status = 400, description = "Invalid request data", body = common_errors::ApiErrorResponse),
        (status = 422, description = "User name already exists", body = common_errors::ApiErrorResponse),
        (status = 500, description = "Internal server error", body = common_errors::ApiErrorResponse)
    ),
    tag = "users"
)]
#[instrument(skip_all)]
pub async fn create_user(
    State(services): State<UserServices>,
    Json(command): Json<CreateUserCommand>,
) -> Result<(StatusCode, Json<UserResponse>), AppError> {
    let result = services.create_user.execute(command).await?;

    tracing::info!("User created: {}", result.id);

    Ok((StatusCode::CREATED, Json(result)))
}

#[utoipa::path(
    put,
    path = "/user/{id}",
    request_body = UpdateUserCommand,
    params(
        ("id" = Uuid, Path, description = "User ID")
    ),
    responses(
        (status = 200, description = "User updated successfully", body = UserResponse),
        (status = 404, description = "User not found", body = common_errors::ApiErrorResponse),
        (status = 400, description = "Invalid request data", body = common_errors::ApiErrorResponse),
        (status = 422, description = "Validation error", body = common_errors::ApiErrorResponse),
        (status = 500, description = "Internal server error", body = common_errors::ApiErrorResponse)
    ),
    tag = "users"
)]
#[instrument(skip_all)]
pub async fn update_user(
    State(services): State<UserServices>, Path(id): Path<Uuid>,
    Json(mut command): Json<UpdateUserCommand>,
) -> Result<Json<UserResponse>, AppError> {
    command.user_id = id;
    let result = services.update_user.execute(command).await?;

    tracing::info!("User updated: {}", id);

    Ok(Json(result))
}

#[utoipa::path(
    delete,
    path = "/user/{id}",
    params(
        ("id" = Uuid, Path, description = "User ID")
    ),
    responses(
        (status = 204, description = "User deleted successfully"),
        (status = 404, description = "User not found", body = common_errors::ApiErrorResponse),
        (status = 500, description = "Internal server error", body = common_errors::ApiErrorResponse)
    ),
    tag = "users"
)]
#[instrument(skip_all)]
pub async fn delete_user(
    State(services): State<UserServices>, Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let command = DeleteUserCommand { user_id: id };
    services.delete_user.execute(command).await?;

    tracing::info!("User deleted: {}", id);

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct UserQueryParams {
    limit: Option<u64>,
    offset: Option<u64>,
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct UserEventsQueryParams {
    limit: Option<u64>,
}

#[utoipa::path(
    get,
    path = "/user/{id}",
    params(
        ("id" = Uuid, Path, description = "User ID"),
        UserQueryParams
    ),
    responses(
        (status = 200, description = "User found", body = UserResponse),
        (status = 400, description = "Invalid UUID format", body = common_errors::ApiErrorResponse),
        (status = 404, description = "User not found", body = common_errors::ApiErrorResponse),
        (status = 500, description = "Internal server error", body = common_errors::ApiErrorResponse)
    ),
    tag = "users"
)]
#[instrument(skip_all)]
pub async fn get_user(
    State(services): State<UserServices>, Path(id): Path<Uuid>,
    Query(_params): Query<UserQueryParams>,
) -> Result<Json<UserResponse>, AppError> {
    let query = user_queries::GetUserQuery { user_id: id };
    let user = services.get_user.execute(query).await?;

    Ok(Json(user.into()))
}

#[utoipa::path(
    get,
    path = "/users",
    params(
        UserQueryParams
    ),
    responses(
        (status = 200, description = "List of users", body = Vec<UserResponse>),
        (status = 400, description = "Invalid query parameters", body = common_errors::ApiErrorResponse),
        (status = 500, description = "Internal server error", body = common_errors::ApiErrorResponse)
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
    let users = services.list_users.execute(query).await?;

    Ok(Json(users.into_iter().map(Into::into).collect()))
}

#[utoipa::path(
    get,
    path = "/user/{user_id}/events",
    params(
        ("user_id" = String, Path, description = "User ID"),
        UserEventsQueryParams
    ),
    responses(
        (status = 200, description = "User events", body = Vec<EventResponse>),
        (status = 400, description = "Invalid UUID format", body = common_errors::ApiErrorResponse),
        (status = 404, description = "User not found", body = common_errors::ApiErrorResponse),
        (status = 500, description = "Internal server error", body = common_errors::ApiErrorResponse)
    ),
    tag = "Event"
)]
#[instrument(skip_all)]
pub async fn get_user_events(
    State(services): State<UserServices>, 
    Path(user_id): Path<String>,
    Query(params): Query<UserEventsQueryParams>,
) -> Result<Json<Vec<EventResponse>>, AppError> {
    let user_uuid = user_id.parse::<Uuid>().map_err(|_| {
        AppError::bad_request("INVALID_UUID", "Invalid UUID format provided")
    })?;

    let query = GetUserEventsQuery {
        user_id: user_uuid,
        limit: params.limit,
    };

    let events = services.get_user_events.execute(query).await?;

    Ok(Json(events.into_iter().map(Into::into).collect()))
}
