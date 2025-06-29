pub mod background_jobs;
pub mod stats;
use axum::{
    Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post, put},
};
use chrono::{DateTime, Utc};
use common_errors::AppError;
use events_command_handlers::{
    BulkDeleteEventsHandler, CreateEventHandler, DeleteEventHandler,
    UpdateEventHandler,
};
use events_commands::{
    BulkDeleteEventsCommand, CreateEventCommand, UpdateEventCommand,
};
use events_queries::{GetEventQuery, ListEventsQuery};
use events_query_handlers::{GetEventQueryHandler, ListEventsQueryHandler};
use events_responses::{BulkDeleteEventsResponse, EventResponse};
use serde::Deserialize;
use sql_connection::SqlConnect;
use tracing::instrument;
use utoipa::{IntoParams, ToSchema};

use crate::{
    background_jobs::BackgroundJobScheduler,
    stats::{StatsService, get_stats},
};

#[derive(Clone)]
pub struct EventServices {
    pub create_event: CreateEventHandler,
    pub update_event: UpdateEventHandler,
    pub delete_event: DeleteEventHandler,
    pub bulk_delete_events: BulkDeleteEventsHandler,

    pub get_event: GetEventQueryHandler,
    pub list_events: ListEventsQueryHandler,
    pub stats: StatsService,
    pub background_jobs: BackgroundJobScheduler,
}

impl EventServices {
    pub fn new(db: SqlConnect) -> Self {
        Self {
            create_event: CreateEventHandler::new(db.clone()),
            update_event: UpdateEventHandler::new(db.clone()),
            delete_event: DeleteEventHandler::new(db.clone()),
            bulk_delete_events: BulkDeleteEventsHandler::new(db.clone()),
            get_event: GetEventQueryHandler::new(db.clone()),
            list_events: ListEventsQueryHandler::new(db.clone()),
            stats: StatsService::new(db.clone()),
            background_jobs: BackgroundJobScheduler::new(db.clone()),
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
            .route("/stats", get(get_stats))
            .route("/{id}", get(get_event))
            .route("/{id}", put(update_event))
            .route("/{id}", delete(delete_event))
    }
}

#[utoipa::path(
    put,
    path = "/event/{id}",
    request_body = UpdateEventCommand,
    params(
        ("id" = i64, Path, description = "Event ID")
    ),
    responses(
        (status = 200, description = "Event updated successfully", body = EventResponse),
        (status = 404, description = "Event not found", body = common_errors::ApiErrorResponse),
        (status = 400, description = "Invalid request data", body = common_errors::ApiErrorResponse),
        (status = 422, description = "Validation error", body = common_errors::ApiErrorResponse),
        (status = 500, description = "Internal server error", body = common_errors::ApiErrorResponse)
    ),
    tag = "events"
)]
#[instrument(skip_all)]
pub async fn update_event(
    State(services): State<EventServices>, Path(id): Path<i64>,
    Json(mut command): Json<UpdateEventCommand>,
) -> Result<Json<EventResponse>, AppError> {
    command.event_id = id;
    let result = services.update_event.execute(command).await?;
    Ok(Json(result))
}

#[utoipa::path(
    delete,
    path = "/event/{id}",
    params(
        ("id" = i64, Path, description = "Event ID")
    ),
    responses(
        (status = 204, description = "Event deleted successfully"),
        (status = 404, description = "Event not found", body = common_errors::ApiErrorResponse),
        (status = 500, description = "Internal server error", body = common_errors::ApiErrorResponse)
    ),
    tag = "events"
)]
#[instrument(skip_all)]
pub async fn delete_event(
    State(services): State<EventServices>, Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    let command = events_commands::DeleteEventCommand { event_id: id };
    services.delete_event.execute(command).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/event/{id}",
    params(
        ("id" = i64, Path, description = "Event ID")
    ),
    responses(
        (status = 200, description = "Event found", body = EventResponse),
        (status = 404, description = "Event not found", body = common_errors::ApiErrorResponse),
        (status = 500, description = "Internal server error", body = common_errors::ApiErrorResponse)
    ),
    tag = "events"
)]
#[instrument(skip_all)]
pub async fn get_event(
    State(services): State<EventServices>, Path(id): Path<i64>,
) -> Result<Json<EventResponse>, AppError> {
    let query = GetEventQuery { event_id: id };
    let event = services.get_event.execute(query).await?;
    Ok(Json(event.into()))
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct EventsListParams {
    pub page: Option<u64>,
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct ListEventsParams {
    pub user_id: Option<i64>,
    pub event_type_id: Option<i32>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
    pub page: Option<u64>,
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct EventsDeleteParams {
    pub before: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct BulkDeleteParams {
    pub before: DateTime<Utc>,
}

#[utoipa::path(
    post,
    path = "/event",
    request_body = CreateEventCommand,
    responses(
        (status = 201, description = "Event created successfully", body = EventResponse),
        (status = 400, description = "Invalid request data", body = common_errors::ApiErrorResponse),
        (status = 422, description = "Validation error", body = common_errors::ApiErrorResponse),
        (status = 500, description = "Internal server error", body = common_errors::ApiErrorResponse)
    ),
    tag = "events"
)]
#[instrument(skip_all)]
pub async fn create_event(
    State(services): State<EventServices>,
    Json(command): Json<CreateEventCommand>,
) -> Result<(StatusCode, Json<EventResponse>), AppError> {
    let result = services.create_event.execute(command).await?;
    Ok((StatusCode::CREATED, Json(result)))
}

#[utoipa::path(
    get,
    path = "/events",
    params(
        ListEventsParams
    ),
    responses(
        (status = 200, description = "List of events", body = Vec<EventResponse>),
        (status = 400, description = "Invalid query parameters", body = common_errors::ApiErrorResponse),
        (status = 500, description = "Internal server error", body = common_errors::ApiErrorResponse)
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
    let events = services.list_events.execute(query).await?;
    Ok(Json(events.into_iter().map(Into::into).collect()))
}

#[utoipa::path(
    delete,
    path = "/events",
    params(
        BulkDeleteParams
    ),
    responses(
        (status = 200, description = "Events deleted successfully", body = BulkDeleteEventsResponse),
        (status = 400, description = "Invalid query parameters", body = common_errors::ApiErrorResponse),
        (status = 500, description = "Internal server error", body = common_errors::ApiErrorResponse)
    ),
    tag = "events"
)]
#[instrument(skip_all)]
pub async fn bulk_delete_events(
    State(services): State<EventServices>,
    Query(params): Query<BulkDeleteParams>,
) -> Result<Json<BulkDeleteEventsResponse>, AppError> {
    let command = BulkDeleteEventsCommand {
        before: params.before,
    };
    let result = services.bulk_delete_events.execute(command).await?;
    Ok(Json(result))
}
