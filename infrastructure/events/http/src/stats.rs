use std::time::Duration;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use chrono::{DateTime, Timelike, Utc};
use common_errors::AppError;
use events_dao::EventDao;
use redis_connection::{
    cache_key, cache_provider::CacheProvider, core::CacheTypeBind,
};
use serde::{Deserialize, Serialize};
use sql_connection::SqlConnect;
use tracing::instrument;
use utoipa::{IntoParams, ToSchema};

use crate::EventServices;

cache_key!(StatsCacheKey::<StatsResponse> => "stats:{}"[cache_key: String]);

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct StatsQuery {
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    #[serde(rename = "type")]
    pub event_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StatsResponse {
    pub total_events: i64,
    pub unique_users: i64,
    pub event_types: Vec<EventTypeStats>,
    pub top_pages: Vec<PageStats>,
    pub time_range: TimeRange,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EventTypeStats {
    pub event_type: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PageStats {
    pub page: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
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

    pub async fn get_stats(
        &self, query: StatsQuery,
    ) -> Result<StatsResponse, AppError> {
        let now = Utc::now();
        let from = query
            .from
            .unwrap_or_else(|| now - chrono::Duration::days(30));
        let to = query.to.unwrap_or(now);

        let backend = CacheProvider::get_backend();
        let cache_key = StatsCacheKey;
        // Round timestamps to hour intervals for better cache hit rates with
        // materialized view
        let from_rounded = from
            .with_minute(0)
            .unwrap()
            .with_second(0)
            .unwrap()
            .with_nanosecond(0)
            .unwrap();
        let to_rounded = to
            .with_minute(0)
            .unwrap()
            .with_second(0)
            .unwrap()
            .with_nanosecond(0)
            .unwrap();

        let event_type_str = query
            .event_type
            .clone()
            .unwrap_or_else(|| "all".to_string());
        let from_str = from_rounded.to_rfc3339();
        let to_str = to_rounded.to_rfc3339();
        let composite_key =
            format!("{}:{}:{}", from_str, to_str, event_type_str);

        let mut cache = cache_key.bind_with(backend.clone(), &composite_key);

        if let Ok(Some(cached_stats)) = cache.try_get().await {
            tracing::debug!(
                "Cache hit for stats query from={} to={} event_type={} \
                 (rounded to hourly intervals)",
                from_str,
                to_str,
                event_type_str
            );
            return Ok(cached_stats);
        }

        tracing::debug!(
            "Cache miss for stats query from={} to={} event_type={} \
             (rounded to hourly intervals), fetching from materialized view",
            from_str,
            to_str,
            event_type_str
        );

        let client =
            self.event_dao.db().get_analytics_client().await.map_err(
                |e| {
                    AppError::internal_server_error(&format!(
                        "Database connection error: {}",
                        e
                    ))
                },
            )?;

        // Use materialized view for much faster queries
        let materialized_view_query = r#"
        WITH filtered_stats AS (
            SELECT 
                key_name as event_type,
                SUM(total_count)::bigint as total_count,
                SUM(unique_users)::bigint as unique_users
            FROM stats_summary
            WHERE stat_type = 'event_type'
            AND hour_bucket >= $2
            AND hour_bucket <= $3
            AND ($1::text IS NULL OR key_name = $1::text)
            GROUP BY key_name
            ORDER BY total_count DESC
        ),
        event_totals AS (
            SELECT 
                COALESCE(SUM(total_count), 0)::bigint as total_events,
                COALESCE(SUM(unique_users), 0)::bigint as total_unique_users
            FROM filtered_stats
        ),
        page_stats AS (
            SELECT 
                key_name as page,
                SUM(page_count)::bigint as count
            FROM stats_summary
            WHERE stat_type = 'page'
            AND hour_bucket >= $2
            AND hour_bucket <= $3
            GROUP BY key_name
            ORDER BY count DESC
            LIMIT 10
        )
        SELECT 
            'events' as result_type,
            event_type,
            total_count as count,
            unique_users,
            NULL::text as page
        FROM filtered_stats
        
        UNION ALL
        
        SELECT 
            'totals' as result_type,
            NULL::text as event_type,
            total_events as count,
            total_unique_users as unique_users,
            NULL::text as page
        FROM event_totals
        
        UNION ALL
        
        SELECT 
            'pages' as result_type,
            NULL::text as event_type,
            count,
            NULL::bigint as unique_users,
            page
        FROM page_stats
        
        ORDER BY result_type, count DESC
    "#;

        // Handle None event_type parameter properly
        let event_type_param: Option<&str> = query.event_type.as_deref();
        let rows = client
            .query(
                materialized_view_query,
                &[&event_type_param, &from_rounded, &to_rounded],
            )
            .await
            .map_err(|e| {
                AppError::internal_server_error(&format!(
                    "Database query error: {}",
                    e
                ))
            })?;

        let mut total_events = 0i64;
        let mut total_unique_users = 0i64;
        let mut event_types = Vec::new();
        let mut top_pages = Vec::new();

        for row in rows {
            let result_type: String = row.get(0);

            match result_type.as_str() {
                "events" => {
                    if let Some(event_type) = row.get::<_, Option<String>>(1)
                    {
                        event_types.push(EventTypeStats {
                            event_type,
                            count: row.get(2),
                        });
                    }
                }
                "pages" => {
                    if let Some(page) = row.get::<_, Option<String>>(4) {
                        top_pages.push(PageStats {
                            page,
                            count: row.get(2),
                        });
                    }
                }
                "totals" => {
                    total_events = row.get(2);
                    total_unique_users =
                        row.get::<_, Option<i64>>(3).unwrap_or(0);
                }
                _ => {} // ignore unknown result types
            }
        }

        let stats_response = StatsResponse {
            total_events,
            unique_users: total_unique_users,
            event_types,
            top_pages,
            time_range: TimeRange { from, to },
        };

        // Cache the result for 15 minutes - materialized view makes this much
        // faster
        let _ = cache
            .set_with_expire::<()>(
                stats_response.clone(),
                Duration::from_secs(900), // 15 minutes
            )
            .await;

        tracing::debug!(
            "Cached stats query from={} to={} event_type={} for 15 minutes \
             (using materialized view)",
            from_str,
            to_str,
            event_type_str
        );

        Ok(stats_response)
    }
}

#[utoipa::path(
    get,
    path = "/stats",
    params(StatsQuery),
    responses(
        (status = 200, description = "Event statistics", body = StatsResponse),
        (status = 400, description = "Invalid query parameters", body = common_errors::ApiErrorResponse),
        (status = 500, description = "Internal server error", body = common_errors::ApiErrorResponse)
    ),
    tag = "stats"
)]
#[instrument(skip_all)]
pub async fn get_stats(
    State(services): State<EventServices>,
    query_result: Result<
        Query<StatsQuery>,
        axum::extract::rejection::QueryRejection,
    >,
) -> Result<Json<StatsResponse>, AppError> {
    let Query(query) = query_result.map_err(|rejection| {
        match rejection {
            axum::extract::rejection::QueryRejection::FailedToDeserializeQueryString(err) => {
                AppError::bad_request_with_details(
                    "INVALID_QUERY_PARAMS",
                    "Invalid query parameters provided",
                    &format!("Query parameter error: {}. Expected date format: RFC3339 (e.g., 2025-01-01T00:00:00Z)", err)
                )
            }
            _ => AppError::bad_request("INVALID_QUERY_PARAMS", "Invalid query parameters provided")
        }
    })?;

    // Validate date range if both dates are provided
    if let (Some(from), Some(to)) = (query.from, query.to) {
        if from >= to {
            return Err(AppError::bad_request(
                "INVALID_DATE_RANGE",
                "The 'from' date must be before the 'to' date",
            ));
        }
    }

    let stats = services.stats.get_stats(query).await?;
    Ok(Json(stats))
}

#[utoipa::path(
    post,
    path = "/stats/refresh",
    responses(
        (status = 200, description = "Stats materialized view refreshed successfully"),
        (status = 500, description = "Internal server error", body = common_errors::ApiErrorResponse)
    ),
    tag = "stats"
)]
#[instrument(skip_all)]
pub async fn refresh_stats(
    State(services): State<EventServices>,
) -> Result<StatusCode, AppError> {
    services.background_jobs.refresh_stats_now().await?;
    Ok(StatusCode::OK)
}
