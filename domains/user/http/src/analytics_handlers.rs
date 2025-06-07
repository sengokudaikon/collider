use axum::{
    Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::instrument;
use user_queries::{UserAnalyticsService, UserEventMetrics};
use uuid::Uuid;

/// Query parameters for analytics endpoints
#[derive(Debug, Deserialize)]
pub struct AnalyticsQuery {
    /// Include metrics in response (default: true)
    #[serde(default = "default_true")]
    pub include_metrics: bool,
    /// Time range for analytics (default: 30 days)
    #[serde(default = "default_30_days")]
    pub days: u32,
    /// Batch size for multi-user requests
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
}

#[derive(Debug, Serialize)]
pub struct UserAnalyticsResponse {
    pub user_id: Uuid,
    pub metrics: UserEventMetrics,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct BatchAnalyticsResponse {
    pub users: Vec<UserAnalyticsResponse>,
    pub total_users: usize,
    pub failed_users: usize,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct BatchUserRequest {
    pub user_ids: Vec<Uuid>,
}

#[derive(Clone)]
pub struct AnalyticsServices {
    pub user_analytics: UserAnalyticsService,
}

impl Default for AnalyticsServices {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalyticsServices {
    pub fn new() -> Self {
        Self {
            user_analytics: UserAnalyticsService::new(),
        }
    }
}

pub struct AnalyticsHandlers;

impl AnalyticsHandlers {
    pub fn routes() -> Router<AnalyticsServices> {
        Router::new()
            .route("/users/:id/analytics", get(get_user_analytics))
            .route("/users/analytics/batch", get(get_batch_analytics))
            .route("/analytics/users", get(get_users_with_analytics))
    }
}

/// Get aggregated analytics for a specific user
/// GET /users/{id}/analytics?days=30&include_metrics=true
#[instrument(skip_all)]
async fn get_user_analytics(
    State(services): State<AnalyticsServices>, Path(user_id): Path<Uuid>,
    Query(_query): Query<AnalyticsQuery>,
) -> Result<Json<UserAnalyticsResponse>, AppError> {
    let metrics = services.user_analytics.get_user_metrics(user_id).await?;

    Ok(Json(UserAnalyticsResponse {
        user_id,
        metrics,
        generated_at: Utc::now(),
    }))
}

/// Get analytics for multiple users efficiently
/// GET /users/analytics/batch?user_ids=uuid1,uuid2,uuid3&batch_size=100
#[instrument(skip_all)]
async fn get_batch_analytics(
    State(services): State<AnalyticsServices>,
    Query(params): Query<BatchUsersQuery>,
) -> Result<Json<BatchAnalyticsResponse>, AppError> {
    let user_ids = params.parse_user_ids()?;
    let batch_size = params.batch_size.unwrap_or(100).min(1000); // Max 1000 users per request

    if user_ids.len() > batch_size {
        return Err(AppError::BadRequest(format!(
            "Too many user IDs. Maximum {} allowed",
            batch_size
        )));
    }

    let results = services
        .user_analytics
        .get_batch_user_metrics(user_ids.clone())
        .await?;
    let failed_count = user_ids.len() - results.len();

    let users = results
        .into_iter()
        .map(|(user_id, metrics)| {
            UserAnalyticsResponse {
                user_id,
                metrics,
                generated_at: Utc::now(),
            }
        })
        .collect();

    Ok(Json(BatchAnalyticsResponse {
        users,
        total_users: user_ids.len(),
        failed_users: failed_count,
        generated_at: Utc::now(),
    }))
}

/// Alternative endpoint: Get users with their analytics in one call
/// Bypasses traditional CRUD and goes straight to aggregated data
/// GET /analytics/users?limit=50&offset=0&include_metrics=true
#[instrument(skip_all)]
async fn get_users_with_analytics(
    State(services): State<AnalyticsServices>,
    Query(query): Query<UsersAnalyticsQuery>,
) -> Result<Json<Vec<UserAnalyticsResponse>>, AppError> {
    use sql_connection::SqlConnect;
    use user_dao::UserDao;

    let db = SqlConnect::default();
    let user_dao = UserDao::new(db);

    let limit = query.limit.unwrap_or(50).min(200);
    let offset = query.offset.unwrap_or(0);

    let users = user_dao
        .find_with_pagination(Some(limit as u64), Some(offset as u64))
        .await
        .map_err(|e| {
            AppError::Internal(format!("Failed to fetch users: {}", e))
        })?;

    if !query.include_metrics.unwrap_or(true) {
        return Ok(Json(
            users
                .into_iter()
                .map(|user| {
                    UserAnalyticsResponse {
                        user_id: user.id,
                        metrics: user_queries::UserEventMetrics {
                            total_events: 0,
                            events_last_24h: 0,
                            events_last_7d: 0,
                            events_last_30d: 0,
                            most_frequent_event_type: None,
                            event_type_counts: vec![],
                        },
                        generated_at: chrono::Utc::now(),
                    }
                })
                .collect(),
        ));
    }

    let user_ids: Vec<uuid::Uuid> = users.iter().map(|u| u.id).collect();
    let metrics_results = services
        .user_analytics
        .get_batch_user_metrics(user_ids)
        .await?;

    let responses = users
        .into_iter()
        .map(|user| {
            let metrics = metrics_results
                .iter()
                .find(|(id, _)| *id == user.id)
                .map(|(_, metrics)| metrics.clone())
                .unwrap_or_else(|| {
                    user_queries::UserEventMetrics {
                        total_events: 0,
                        events_last_24h: 0,
                        events_last_7d: 0,
                        events_last_30d: 0,
                        most_frequent_event_type: None,
                        event_type_counts: vec![],
                    }
                });

            UserAnalyticsResponse {
                user_id: user.id,
                metrics,
                generated_at: chrono::Utc::now(),
            }
        })
        .collect();

    Ok(Json(responses))
}

#[derive(Debug, Deserialize)]
struct BatchUsersQuery {
    /// Comma-separated list of UUIDs
    user_ids: String,
    batch_size: Option<usize>,
}

impl BatchUsersQuery {
    fn parse_user_ids(&self) -> Result<Vec<Uuid>, AppError> {
        self.user_ids
            .split(',')
            .map(|s| s.trim().parse::<Uuid>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                AppError::BadRequest(format!("Invalid UUID format: {}", e))
            })
    }
}

#[derive(Debug, Deserialize)]
struct UsersAnalyticsQuery {
    limit: Option<usize>,
    offset: Option<usize>,
    include_metrics: Option<bool>,
}

fn default_true() -> bool { true }
fn default_30_days() -> u32 { 30 }
fn default_batch_size() -> usize { 100 }

#[derive(Debug)]
pub enum AppError {
    Analytics(user_queries::UserAnalyticsError),
    BadRequest(String),
    Internal(String),
}

impl<E> From<E> for AppError
where
    E: Into<user_queries::UserAnalyticsError>,
{
    fn from(err: E) -> Self { Self::Analytics(err.into()) }
}

impl axum::response::IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            AppError::Analytics(e) => {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Analytics error: {}", e),
                )
            }
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::Internal(msg) => {
                (StatusCode::INTERNAL_SERVER_ERROR, msg)
            }
        };
        (status, message).into_response()
    }
}
