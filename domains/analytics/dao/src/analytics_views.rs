use analytics_commands::{
    RefreshViewsCommand,
    refresh_views::{RefreshViewsError, RefreshViewsResponse},
};
use analytics_models::{
    EventHourlySummary, EventMetrics, PopularEvent, UserDailyActivity,
    UserMetrics,
};
use chrono::{DateTime, Utc};
use sql_connection::SqlConnect;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum AnalyticsViewsDaoError {
    #[error("Database error: {0}")]
    Database(#[from] tokio_postgres::Error),
    #[error("Connection error: {0}")]
    Connection(String),
    #[error("View refresh error: {0}")]
    ViewRefresh(#[from] RefreshViewsError),
}

pub struct AnalyticsViewsDao {
    db: SqlConnect,
}

impl AnalyticsViewsDao {
    pub fn new(db: SqlConnect) -> Self { Self { db } }

    pub async fn refresh_views(
        &self, command: RefreshViewsCommand,
    ) -> Result<RefreshViewsResponse, AnalyticsViewsDaoError> {
        let client =
            self.db.get_client().await.map_err(|e| {
                AnalyticsViewsDaoError::Connection(e.to_string())
            })?;

        let start_time = std::time::Instant::now();
        let mut refreshed_views = Vec::new();

        let views_to_refresh = if let Some(view_name) = command.view_name {
            vec![view_name]
        }
        else {
            vec![
                "event_hourly_summaries".to_string(),
                "user_daily_activity".to_string(),
                "popular_events".to_string(),
                "user_session_summaries".to_string(),
                "page_analytics".to_string(),
                "product_analytics".to_string(),
                "referrer_analytics".to_string(),
            ]
        };

        let concurrent_clause = if command.concurrent {
            "CONCURRENTLY"
        }
        else {
            ""
        };

        for view_name in views_to_refresh {
            let sql = format!(
                "REFRESH MATERIALIZED VIEW {} {}",
                concurrent_clause, view_name
            );
            client.execute(&sql, &[]).await?;
            refreshed_views.push(view_name);
        }

        let duration_ms = start_time.elapsed().as_millis() as u64;

        Ok(RefreshViewsResponse {
            refreshed_views,
            duration_ms,
        })
    }

    pub async fn get_event_hourly_summaries(
        &self, start_time: DateTime<Utc>, end_time: DateTime<Utc>,
        event_types: Option<Vec<String>>, limit: Option<i64>,
    ) -> Result<Vec<EventHourlySummary>, AnalyticsViewsDaoError> {
        let client =
            self.db.get_client().await.map_err(|e| {
                AnalyticsViewsDaoError::Connection(e.to_string())
            })?;

        let mut sql = "SELECT event_type, hour, total_events, unique_users 
                       FROM event_hourly_summaries 
                       WHERE hour >= $1 AND hour <= $2"
            .to_string();
        let mut params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
            vec![&start_time, &end_time];

        if let Some(types) = &event_types {
            sql.push_str(" AND event_type = ANY($3)");
            params.push(types);
        }

        sql.push_str(" ORDER BY hour DESC, total_events DESC");

        let limit_param;
        if let Some(limit_val) = limit {
            limit_param = limit_val;
            if event_types.is_some() {
                sql.push_str(" LIMIT $4");
                params.push(&limit_param);
            }
            else {
                sql.push_str(" LIMIT $3");
                params.push(&limit_param);
            }
        }

        let rows = client.query(&sql, &params).await?;

        let summaries = rows
            .iter()
            .map(|row| {
                EventHourlySummary {
                    event_type: row.get("event_type"),
                    hour: row.get("hour"),
                    total_events: row.get("total_events"),
                    unique_users: row.get("unique_users"),
                }
            })
            .collect();

        Ok(summaries)
    }

    pub async fn get_user_daily_activity(
        &self, user_id: Option<Uuid>, start_date: DateTime<Utc>,
        end_date: DateTime<Utc>, limit: Option<i64>,
    ) -> Result<Vec<UserDailyActivity>, AnalyticsViewsDaoError> {
        let client =
            self.db.get_client().await.map_err(|e| {
                AnalyticsViewsDaoError::Connection(e.to_string())
            })?;

        let mut sql = "SELECT user_id, date, total_events, \
                       unique_event_types, first_event, last_event 
                       FROM user_daily_activity 
                       WHERE date >= $1 AND date <= $2"
            .to_string();
        let mut params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
            vec![&start_date, &end_date];

        if let Some(uid) = &user_id {
            sql.push_str(" AND user_id = $3");
            params.push(uid);
        }

        sql.push_str(" ORDER BY date DESC, total_events DESC");

        let limit_param;
        if let Some(limit_val) = limit {
            limit_param = limit_val;
            if user_id.is_some() {
                sql.push_str(" LIMIT $4");
                params.push(&limit_param);
            }
            else {
                sql.push_str(" LIMIT $3");
                params.push(&limit_param);
            }
        }

        let rows = client.query(&sql, &params).await?;

        let activities = rows
            .iter()
            .map(|row| {
                UserDailyActivity {
                    user_id: row.get("user_id"),
                    date: row.get("date"),
                    total_events: row.get("total_events"),
                    unique_event_types: row.get("unique_event_types"),
                    first_event: row.get("first_event"),
                    last_event: row.get("last_event"),
                }
            })
            .collect();

        Ok(activities)
    }

    pub async fn get_popular_events(
        &self, period: Option<String>, limit: Option<i64>,
    ) -> Result<Vec<PopularEvent>, AnalyticsViewsDaoError> {
        let client =
            self.db.get_client().await.map_err(|e| {
                AnalyticsViewsDaoError::Connection(e.to_string())
            })?;

        let mut sql = "SELECT event_type, period, total_count, \
                       unique_users, growth_rate 
                       FROM popular_events"
            .to_string();
        let mut params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
            vec![];

        if let Some(p) = &period {
            sql.push_str(" WHERE period = $1");
            params.push(p);
        }

        sql.push_str(" ORDER BY total_count DESC");

        let limit_param;
        if let Some(limit_val) = limit {
            limit_param = limit_val;
            if period.is_some() {
                sql.push_str(" LIMIT $2");
                params.push(&limit_param);
            }
            else {
                sql.push_str(" LIMIT $1");
                params.push(&limit_param);
            }
        }

        let rows = client.query(&sql, &params).await?;

        let events = rows
            .iter()
            .map(|row| {
                PopularEvent {
                    event_type: row.get("event_type"),
                    period: row.get("period"),
                    total_count: row.get("total_count"),
                    unique_users: row.get("unique_users"),
                    growth_rate: row.get("growth_rate"),
                }
            })
            .collect();

        Ok(events)
    }

    /// Get event metrics using materialized views
    pub async fn get_event_metrics(
        &self, start: DateTime<Utc>, end: DateTime<Utc>,
        event_type_filter: Option<String>,
    ) -> Result<EventMetrics, AnalyticsViewsDaoError> {
        let client =
            self.db.get_client().await.map_err(|e| {
                AnalyticsViewsDaoError::Connection(e.to_string())
            })?;

        // Get basic metrics from hourly summaries
        let base_query = if event_type_filter.is_some() {
            "SELECT COALESCE(SUM(total_events), 0)::bigint as total_events, 
                    COUNT(DISTINCT unique_users) as unique_users
             FROM event_hourly_summaries 
             WHERE hour >= $1 AND hour <= $2 AND event_type = $3"
        }
        else {
            "SELECT COALESCE(SUM(total_events), 0)::bigint as total_events, 
                    COUNT(DISTINCT unique_users) as unique_users
             FROM event_hourly_summaries 
             WHERE hour >= $1 AND hour <= $2"
        };

        let stmt = client.prepare(base_query).await?;
        let rows = if let Some(ref event_type) = event_type_filter {
            client.query(&stmt, &[&start, &end, event_type]).await?
        }
        else {
            client.query(&stmt, &[&start, &end]).await?
        };

        let row = rows.first().ok_or_else(|| {
            AnalyticsViewsDaoError::Database(
                tokio_postgres::Error::__private_api_timeout(),
            )
        })?;

        let total_events: i64 = row.get(0);
        let unique_users: i64 = row.get(1);
        let events_per_user = if unique_users > 0 {
            total_events as f64 / unique_users as f64
        }
        else {
            0.0
        };

        // Get top events from popular_events view
        let top_events_query = if event_type_filter.is_some() {
            "SELECT event_type, total_count as count
             FROM popular_events 
             WHERE event_type = $1
             ORDER BY total_count DESC 
             LIMIT 10"
        }
        else {
            "SELECT event_type, total_count as count
             FROM popular_events 
             ORDER BY total_count DESC 
             LIMIT 10"
        };

        let stmt = client.prepare(top_events_query).await?;
        let rows = if let Some(ref event_type) = event_type_filter {
            client.query(&stmt, &[event_type]).await?
        }
        else {
            client.query(&stmt, &[]).await?
        };

        let top_events = rows
            .iter()
            .map(|row| {
                let count: i64 = row.get(1);
                let percentage = if total_events > 0 {
                    (count as f64 / total_events as f64) * 100.0
                }
                else {
                    0.0
                };
                analytics_models::EventTypeCount {
                    event_type: row.get(0),
                    count,
                    percentage,
                }
            })
            .collect();

        Ok(EventMetrics {
            total_events,
            unique_users,
            events_per_user,
            period_start: start,
            period_end: end,
            top_events,
        })
    }

    /// Get user metrics using materialized views and base events table
    pub async fn get_user_metrics(
        &self, user_id: Uuid, start: DateTime<Utc>, end: DateTime<Utc>,
    ) -> Result<UserMetrics, AnalyticsViewsDaoError> {
        let client =
            self.db.get_client().await.map_err(|e| {
                AnalyticsViewsDaoError::Connection(e.to_string())
            })?;

        // Get basic user metrics from user_daily_activity view
        let activity_query = "SELECT COALESCE(SUM(total_events), 0)::bigint \
                              as total_events,
                                     MIN(first_event) as first_seen,
                                     MAX(last_event) as last_seen
                              FROM user_daily_activity 
                              WHERE user_id = $1 AND date >= $2 AND date <= \
                              $3";

        let stmt = client.prepare(activity_query).await?;
        let rows = client.query(&stmt, &[&user_id, &start, &end]).await?;

        let row = rows.first().ok_or_else(|| {
            AnalyticsViewsDaoError::Database(
                tokio_postgres::Error::__private_api_timeout(),
            )
        })?;

        let total_events: i64 = row.get(0);
        let first_seen: Option<DateTime<Utc>> = row.get(1);
        let last_seen: Option<DateTime<Utc>> = row.get(2);

        // If no events found, return default metrics
        if total_events == 0 {
            return Ok(UserMetrics {
                user_id,
                total_events: 0,
                total_sessions: 0,
                total_time_spent: 0,
                avg_session_duration: 0.0,
                first_seen: start, // Use query bounds as defaults
                last_seen: end,
                most_active_day: "N/A".to_string(),
                favorite_events: vec![],
            });
        }

        // Get session metrics from user_session_summaries view
        let session_query = "SELECT total_sessions, avg_session_duration, \
                             total_time_spent
                             FROM user_session_summaries 
                             WHERE user_id = $1";

        let stmt = client.prepare(session_query).await?;
        let rows = client.query(&stmt, &[&user_id]).await?;

        let (total_sessions, avg_session_duration, total_time_spent) =
            if let Some(row) = rows.first() {
                let sessions: i64 = row.get(0);
                let avg_duration: f64 = row.get(1);
                let total_time: f64 = row.get(2);
                (sessions, avg_duration, total_time as i64)
            }
            else {
                (0, 0.0, 0)
            };

        // Get favorite events from base events table
        let favorite_events_query = "SELECT et.name as event_type, COUNT(*) \
                                     as count
                                    FROM events e
                                    JOIN event_types et ON e.event_type_id = \
                                     et.id
                                    WHERE e.user_id = $1 AND e.timestamp >= \
                                     $2 AND e.timestamp <= $3
                                    GROUP BY et.name 
                                    ORDER BY count DESC 
                                    LIMIT 5";

        let stmt = client.prepare(favorite_events_query).await?;
        let rows = client.query(&stmt, &[&user_id, &start, &end]).await?;

        let favorite_events = rows
            .iter()
            .map(|row| {
                let count: i64 = row.get(1);
                let percentage = if total_events > 0 {
                    (count as f64 / total_events as f64) * 100.0
                }
                else {
                    0.0
                };
                analytics_models::EventTypeCount {
                    event_type: row.get(0),
                    count,
                    percentage,
                }
            })
            .collect();

        // Get most active day of week from base events table
        let active_day_query = "SELECT EXTRACT(DOW FROM e.timestamp) as \
                                day_of_week, COUNT(*) as count
                               FROM events e
                               WHERE e.user_id = $1 AND e.timestamp >= $2 \
                                AND e.timestamp <= $3
                               GROUP BY EXTRACT(DOW FROM e.timestamp) 
                               ORDER BY count DESC 
                               LIMIT 1";

        let stmt = client.prepare(active_day_query).await?;
        let rows = client.query(&stmt, &[&user_id, &start, &end]).await?;

        let most_active_day = if let Some(row) = rows.first() {
            let dow: f64 = row.get(0);
            match dow as i32 {
                0 => "Sunday",
                1 => "Monday",
                2 => "Tuesday",
                3 => "Wednesday",
                4 => "Thursday",
                5 => "Friday",
                6 => "Saturday",
                _ => "Unknown",
            }
            .to_string()
        }
        else {
            "Unknown".to_string()
        };

        Ok(UserMetrics {
            user_id,
            total_events,
            total_sessions,
            total_time_spent,
            avg_session_duration,
            first_seen: first_seen.unwrap_or(start),
            last_seen: last_seen.unwrap_or(end),
            most_active_day,
            favorite_events,
        })
    }

    pub async fn get_user_session_summaries(
        &self, user_id: Option<Uuid>, limit: Option<i64>,
    ) -> Result<
        Vec<analytics_models::UserSessionSummary>,
        AnalyticsViewsDaoError,
    > {
        let client =
            self.db.get_client().await.map_err(|e| {
                AnalyticsViewsDaoError::Connection(e.to_string())
            })?;

        let mut sql = "SELECT user_id, total_sessions, \
                       avg_session_duration, total_time_spent, \
                       avg_events_per_session, first_session, last_session 
                       FROM user_session_summaries"
            .to_string();
        let mut params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
            vec![];

        if let Some(uid) = &user_id {
            sql.push_str(" WHERE user_id = $1");
            params.push(uid);
        }

        sql.push_str(" ORDER BY total_sessions DESC");

        let limit_param;
        if let Some(limit_val) = limit {
            limit_param = limit_val;
            if user_id.is_some() {
                sql.push_str(" LIMIT $2");
                params.push(&limit_param);
            }
            else {
                sql.push_str(" LIMIT $1");
                params.push(&limit_param);
            }
        }

        let rows = client.query(&sql, &params).await?;

        let summaries = rows
            .iter()
            .map(|row| {
                analytics_models::UserSessionSummary {
                    user_id: row.get("user_id"),
                    total_sessions: row.get("total_sessions"),
                    avg_session_duration: row.get("avg_session_duration"),
                    total_time_spent: row.get("total_time_spent"),
                    avg_events_per_session: row.get("avg_events_per_session"),
                    first_session: row.get("first_session"),
                    last_session: row.get("last_session"),
                }
            })
            .collect();

        Ok(summaries)
    }

    pub async fn get_page_analytics(
        &self, page: Option<String>, start_time: DateTime<Utc>,
        end_time: DateTime<Utc>, limit: Option<i64>,
    ) -> Result<Vec<analytics_models::PageAnalytics>, AnalyticsViewsDaoError>
    {
        let client =
            self.db.get_client().await.map_err(|e| {
                AnalyticsViewsDaoError::Connection(e.to_string())
            })?;

        let mut sql = "SELECT page, hour, total_events, unique_users, \
                       unique_sessions 
                       FROM page_analytics 
                       WHERE hour >= $1 AND hour <= $2"
            .to_string();
        let mut params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
            vec![&start_time, &end_time];

        if let Some(p) = &page {
            sql.push_str(" AND page = $3");
            params.push(p);
        }

        sql.push_str(" ORDER BY hour DESC, total_events DESC");

        let limit_param;
        if let Some(limit_val) = limit {
            limit_param = limit_val;
            if page.is_some() {
                sql.push_str(" LIMIT $4");
                params.push(&limit_param);
            }
            else {
                sql.push_str(" LIMIT $3");
                params.push(&limit_param);
            }
        }

        let rows = client.query(&sql, &params).await?;

        let analytics = rows
            .iter()
            .map(|row| {
                analytics_models::PageAnalytics {
                    page: row.get("page"),
                    hour: row.get("hour"),
                    total_events: row.get("total_events"),
                    unique_users: row.get("unique_users"),
                    unique_sessions: row.get("unique_sessions"),
                }
            })
            .collect();

        Ok(analytics)
    }

    pub async fn get_product_analytics(
        &self, product_id: Option<i32>, event_type: Option<String>,
        start_date: DateTime<Utc>, end_date: DateTime<Utc>,
        limit: Option<i64>,
    ) -> Result<Vec<analytics_models::ProductAnalytics>, AnalyticsViewsDaoError>
    {
        let client =
            self.db.get_client().await.map_err(|e| {
                AnalyticsViewsDaoError::Connection(e.to_string())
            })?;

        let mut sql = "SELECT product_id, event_type, date, total_events, \
                       unique_users 
                       FROM product_analytics 
                       WHERE date >= $1 AND date <= $2"
            .to_string();
        let mut params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
            vec![&start_date, &end_date];

        if let Some(pid) = &product_id {
            sql.push_str(" AND product_id = $3");
            params.push(pid);
        }

        if let Some(et) = &event_type {
            if product_id.is_some() {
                sql.push_str(" AND event_type = $4");
            }
            else {
                sql.push_str(" AND event_type = $3");
            }
            params.push(et);
        }

        sql.push_str(" ORDER BY date DESC, total_events DESC");

        let limit_param;
        if let Some(limit_val) = limit {
            limit_param = limit_val;
            let param_index = params.len() + 1;
            sql.push_str(&format!(" LIMIT ${}", param_index));
            params.push(&limit_param);
        }

        let rows = client.query(&sql, &params).await?;

        let analytics = rows
            .iter()
            .map(|row| {
                analytics_models::ProductAnalytics {
                    product_id: row.get("product_id"),
                    event_type: row.get("event_type"),
                    date: row.get("date"),
                    total_events: row.get("total_events"),
                    unique_users: row.get("unique_users"),
                }
            })
            .collect();

        Ok(analytics)
    }

    pub async fn get_referrer_analytics(
        &self, referrer: Option<String>, start_date: DateTime<Utc>,
        end_date: DateTime<Utc>, limit: Option<i64>,
    ) -> Result<
        Vec<analytics_models::ReferrerAnalytics>,
        AnalyticsViewsDaoError,
    > {
        let client =
            self.db.get_client().await.map_err(|e| {
                AnalyticsViewsDaoError::Connection(e.to_string())
            })?;

        let mut sql = "SELECT referrer, date, total_events, unique_users, \
                       unique_sessions 
                       FROM referrer_analytics 
                       WHERE date >= $1 AND date <= $2"
            .to_string();
        let mut params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
            vec![&start_date, &end_date];

        if let Some(r) = &referrer {
            sql.push_str(" AND referrer = $3");
            params.push(r);
        }

        sql.push_str(" ORDER BY date DESC, total_events DESC");

        let limit_param;
        if let Some(limit_val) = limit {
            limit_param = limit_val;
            if referrer.is_some() {
                sql.push_str(" LIMIT $4");
                params.push(&limit_param);
            }
            else {
                sql.push_str(" LIMIT $3");
                params.push(&limit_param);
            }
        }

        let rows = client.query(&sql, &params).await?;

        let analytics = rows
            .iter()
            .map(|row| {
                analytics_models::ReferrerAnalytics {
                    referrer: row.get("referrer"),
                    date: row.get("date"),
                    total_events: row.get("total_events"),
                    unique_users: row.get("unique_users"),
                    unique_sessions: row.get("unique_sessions"),
                }
            })
            .collect();

        Ok(analytics)
    }
}
