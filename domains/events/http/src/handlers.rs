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
