use axum::{
    extract::{Query, State},
    response::Json,
};
use chrono::{DateTime, Utc};
use domain::AppError;
use events_dao::EventDao;
use serde::{Deserialize, Serialize};
use sql_connection::SqlConnect;
use tracing::instrument;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct StatsQuery {
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    #[serde(rename = "type")]
    pub event_type: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct StatsResponse {
    pub total_events: i64,
    pub unique_users: i64,
    pub event_types: Vec<EventTypeStats>,
    pub time_range: TimeRange,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct EventTypeStats {
    pub event_type: String,
    pub count: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TimeRange {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
}

#[derive(Clone)]
pub struct StatsService {
    event_dao: EventDao,
}

impl StatsService {
    pub fn new(db: SqlConnect) -> Self {
        Self {
            event_dao: EventDao::new(db),
        }
    }

    pub async fn get_stats(&self, query: StatsQuery) -> Result<StatsResponse, AppError> {
        let now = Utc::now();
        let from = query.from.unwrap_or_else(|| now - chrono::Duration::days(30));
        let to = query.to.unwrap_or(now);

        // Get total events count
        let total_events = self.event_dao.count_events(from, to, query.event_type.clone()).await
            .map_err(AppError::from_error)?;

        // Get unique users count
        let unique_users = self.event_dao.count_unique_users(from, to, query.event_type.clone()).await
            .map_err(AppError::from_error)?;

        // Get event type stats
        let event_types = self.event_dao.get_event_type_stats(from, to, query.event_type).await
            .map_err(AppError::from_error)?
            .into_iter()
            .map(|(event_type, count)| EventTypeStats { event_type, count })
            .collect();

        Ok(StatsResponse {
            total_events,
            unique_users,
            event_types,
            time_range: TimeRange { from, to },
        })
    }
}

#[utoipa::path(
    get,
    path = "/api/events/stats",
    params(StatsQuery),
    responses(
        (status = 200, description = "Event statistics", body = StatsResponse),
        (status = 400, description = "Invalid query parameters"),
        (status = 500, description = "Internal server error")
    ),
    tag = "events"
)]
#[instrument(skip_all)]
pub async fn get_stats(
    State(services): State<crate::handlers::EventServices>,
    Query(query): Query<StatsQuery>,
) -> Result<Json<StatsResponse>, AppError> {
    let stats = services.stats.get_stats(query).await?;
    Ok(Json(stats))
}