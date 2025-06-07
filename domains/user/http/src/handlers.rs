use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post, put},
    Router,
};
use event_bus::{EventBus, SystemEvent};
use serde::Deserialize;
use tracing::instrument;
use user_commands::{
    CreateUserCommand, CreateUserHandler, DeleteUserCommand,
    DeleteUserHandler, UpdateUserCommand, UpdateUserHandler,
};
use user_queries::{
    GetUserByNameQueryHandler, GetUserQueryHandler, ListUsersQueryHandler,
    UserAnalyticsService,
};
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
    pub analytics: UserAnalyticsService,
    pub event_bus: EventBus<SystemEvent>,
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
            analytics: UserAnalyticsService::new(),
            event_bus: EventBus::new(1000),
        }
    }
}

pub struct UserHandlers;

impl UserHandlers {
    pub fn routes() -> Router<UserServices> {
        Router::new()
            .route("/", get(list_users))
            .route("/", post(create_user))
            .route("/:id", get(get_user))
            .route("/:id", put(update_user))
            .route("/:id", delete(delete_user))
            .route("/by-name/:username", get(get_user_by_name))
            .route("/:id/metrics", get(get_user_with_metrics))
    }
}

// Command handlers
#[instrument(skip_all)]
async fn create_user(
    State(services): State<UserServices>,
    Json(command): Json<CreateUserCommand>,
) -> Result<(StatusCode, Json<user_commands::CreateUserResponse>), AppError> {
    let result = services.create_user.execute(command).await?;

    for event in &result.events {
        if event.event_type == "user_created" {
            if let Err(e) = services
                .event_bus
                .publish(
                    "user_created",
                    format!("user_{}", event.user_id),
                    SystemEvent::UserCreated {
                        user_id: event.user_id,
                    },
                    None,
                    None,
                )
                .await
            {
                tracing::warn!("Failed to publish user created event: {}", e);
            }
        }
    }

    Ok((StatusCode::CREATED, Json(result.user)))
}

#[instrument(skip_all)]
async fn update_user(
    State(services): State<UserServices>, Path(id): Path<Uuid>,
    Json(mut command): Json<UpdateUserCommand>,
) -> Result<Json<user_commands::UpdateUserResponse>, AppError> {
    command.user_id = id;
    let result = services.update_user.execute(command).await?;

    if let Err(e) = services
        .event_bus
        .publish(
            "user_updated",
            format!("user_{}", id),
            SystemEvent::UserUpdated {
                user_id: id,
                fields: vec!["name".to_string()],
            },
            None,
            None,
        )
        .await
    {
        tracing::warn!("Failed to publish user updated event: {}", e);
    }

    Ok(Json(result.user))
}

#[instrument(skip_all)]
async fn delete_user(
    State(services): State<UserServices>, Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let command = DeleteUserCommand { user_id: id };
    services.delete_user.execute(command).await?;

    if let Err(e) = services
        .event_bus
        .publish(
            "user_deleted",
            format!("user_{}", id),
            SystemEvent::UserDeleted { user_id: id },
            None,
            None,
        )
        .await
    {
        tracing::warn!("Failed to publish user deleted event: {}", e);
    }

    Ok(StatusCode::NO_CONTENT)
}

/// Query parameters for user endpoints
#[derive(Debug, Deserialize)]
struct UserQueryParams {
    #[serde(default)]
    include_metrics: bool,
    limit: Option<u64>,
    offset: Option<u64>,
}

// Query handlers
#[instrument(skip_all)]
async fn get_user(
    State(services): State<UserServices>, Path(id): Path<Uuid>,
    Query(params): Query<UserQueryParams>,
) -> Result<Json<UserResponse>, AppError> {
    let query = user_queries::GetUserQuery { user_id: id };
    let user = services.get_user.execute(query).await?;

    if params.include_metrics {
        // Try to get metrics, fallback to basic user if fails
        match services.analytics.get_user_metrics(id).await {
            Ok(metrics) => {
                let response =
                    UserResponse::with_metrics(user, metrics);
                Ok(Json(response))
            }
            Err(_) => Ok(Json(user.into())),
        }
    }
    else {
        Ok(Json(user.into()))
    }
}

#[instrument(skip_all)]
async fn list_users(
    State(services): State<UserServices>,
    Query(params): Query<UserQueryParams>,
) -> Result<Json<Vec<UserResponse>>, AppError> {
    let query = user_queries::ListUsersQuery {
        limit: params.limit,
        offset: params.offset,
    };
    let users = services.list_users.execute(query).await?;

    if params.include_metrics {
        // Get batch analytics for all users
        let user_ids: Vec<Uuid> = users.iter().map(|u| u.id).collect();
        match services.analytics.get_batch_user_metrics(user_ids).await {
            Ok(metrics_map) => {
                let responses = users
                    .into_iter()
                    .map(|user| {
                        if let Some((_, metrics)) =
                            metrics_map.iter().find(|(id, _)| *id == user.id)
                        {
                            UserResponse::with_metrics(user, metrics.clone())
                        }
                        else {
                            user.into()
                        }
                    })
                    .collect();
                Ok(Json(responses))
            }
            Err(_) => {
                // Fallback to users without metrics
                Ok(Json(users.into_iter().map(Into::into).collect()))
            }
        }
    }
    else {
        Ok(Json(users.into_iter().map(Into::into).collect()))
    }
}

// Get user by username
#[instrument(skip_all)]
async fn get_user_by_name(
    State(services): State<UserServices>, Path(username): Path<String>,
) -> Result<Json<UserResponse>, AppError> {
    let query = user_queries::GetUserByNameQuery { username };
    let user = services.get_user_by_name.execute(query).await?;
    Ok(Json(user.into()))
}

// Get user with aggregated metrics
#[instrument(skip_all)]
async fn get_user_with_metrics(
    State(services): State<UserServices>, Path(id): Path<Uuid>,
) -> Result<Json<UserResponse>, AppError> {
    let user_query = user_queries::GetUserQuery { user_id: id };
    let user = services.get_user.execute(user_query).await?;

    match services.analytics.get_user_metrics(id).await {
        Ok(metrics) => {
            let response = UserResponse::with_metrics(user, metrics);
            Ok(Json(response))
        }
        Err(_) => Ok(Json(user.into())),
    }
}

#[derive(Debug)]
struct AppError(Box<dyn std::error::Error + Send + Sync>);

impl<E> From<E> for AppError
where
    E: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    fn from(err: E) -> Self { Self(err.into()) }
}

impl axum::response::IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let status = StatusCode::INTERNAL_SERVER_ERROR;
        let message = format!("Internal server error: {}", self.0);
        (status, message).into_response()
    }
}
