use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{sea_query::*, *};
use serde::{Deserialize, Serialize};
use sql_connection::{
    SqlConnect, database_traits::connection::GetDatabaseConnect,
};
use thiserror::Error;
use tracing::{info, instrument};
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum MaterializedViewError {
    #[error("Database error: {0}")]
    Database(#[from] DbErr),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSummary {
    pub event_type: String,
    pub hour: DateTime<Utc>,
    pub total_events: i64,
    pub unique_users: i64,
    pub avg_events_per_user: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserActivity {
    pub user_id: Uuid,
    pub date: DateTime<Utc>,
    pub total_events: i64,
    pub event_types: Vec<String>,
    pub first_event: DateTime<Utc>,
    pub last_event: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PopularEvents {
    pub event_type: String,
    pub period: String,
    pub total_count: i64,
    pub unique_users: i64,
    pub growth_rate: Option<f64>,
}

#[async_trait]
pub trait MaterializedViewManager: Send + Sync {
    async fn refresh_hourly_summaries(
        &self,
    ) -> Result<(), MaterializedViewError>;
    async fn refresh_daily_user_activity(
        &self,
    ) -> Result<(), MaterializedViewError>;
    async fn refresh_popular_events(
        &self,
    ) -> Result<(), MaterializedViewError>;

    async fn get_hourly_summaries(
        &self, start: DateTime<Utc>, end: DateTime<Utc>,
        event_types: Option<Vec<String>>,
    ) -> Result<Vec<EventSummary>, MaterializedViewError>;

    async fn get_user_activity(
        &self, user_id: Option<Uuid>, start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<UserActivity>, MaterializedViewError>;

    async fn get_popular_events(
        &self, period: &str, limit: Option<i64>,
    ) -> Result<Vec<PopularEvents>, MaterializedViewError>;
}

pub struct PostgresMaterializedViewManager {
    db: SqlConnect,
}

impl PostgresMaterializedViewManager {
    pub fn new(db: SqlConnect) -> Self { Self { db } }
}

#[async_trait]
impl MaterializedViewManager for PostgresMaterializedViewManager {
    #[instrument(skip(self))]
    async fn refresh_hourly_summaries(
        &self,
    ) -> Result<(), MaterializedViewError> {
        let db = self.db.get_connect();
        db.execute_unprepared(
            "REFRESH MATERIALIZED VIEW CONCURRENTLY event_hourly_summaries;",
        )
        .await?;

        info!("Refreshed hourly summaries materialized view");
        Ok(())
    }

    #[instrument(skip(self))]
    async fn refresh_daily_user_activity(
        &self,
    ) -> Result<(), MaterializedViewError> {
        let db = self.db.get_connect();
        db.execute_unprepared(
            "REFRESH MATERIALIZED VIEW CONCURRENTLY user_daily_activity;",
        )
        .await?;

        info!("Refreshed daily user activity materialized view");
        Ok(())
    }

    #[instrument(skip(self))]
    async fn refresh_popular_events(
        &self,
    ) -> Result<(), MaterializedViewError> {
        let db = self.db.get_connect();
        db.execute_unprepared(
            "REFRESH MATERIALIZED VIEW CONCURRENTLY popular_events;",
        )
        .await?;

        info!("Refreshed popular events materialized view");
        Ok(())
    }

    async fn get_hourly_summaries(
        &self, start: DateTime<Utc>, end: DateTime<Utc>,
        event_types: Option<Vec<String>>,
    ) -> Result<Vec<EventSummary>, MaterializedViewError> {
        let db = self.db.get_connect();

        let mut query = "SELECT event_type, hour, total_events, \
                         unique_users, avg_events_per_user 
                        FROM event_hourly_summaries 
                        WHERE hour >= $1 AND hour <= $2"
            .to_string();

        let mut params: Vec<Value> = vec![start.into(), end.into()];

        if let Some(types) = event_types {
            query.push_str(" AND event_type = ANY($3)");
            params.push(types.into());
        }

        query.push_str(" ORDER BY hour DESC, total_events DESC");

        let stmt = Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            query,
            params,
        );
        let rows = db.query_all(stmt).await?;

        let mut summaries = Vec::new();
        for row in rows {
            summaries.push(EventSummary {
                event_type: row.try_get("", "event_type")?,
                hour: row.try_get("", "hour")?,
                total_events: row.try_get("", "total_events")?,
                unique_users: row.try_get("", "unique_users")?,
                avg_events_per_user: row
                    .try_get("", "avg_events_per_user")?,
            });
        }

        Ok(summaries)
    }

    async fn get_user_activity(
        &self, user_id: Option<Uuid>, start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<UserActivity>, MaterializedViewError> {
        let db = self.db.get_connect();

        let mut query = "SELECT user_id, date, total_events, event_types, \
                         first_event, last_event 
                        FROM user_daily_activity 
                        WHERE date >= $1 AND date <= $2"
            .to_string();

        let mut params: Vec<Value> = vec![start.into(), end.into()];

        if let Some(uid) = user_id {
            query.push_str(" AND user_id = $3");
            params.push(uid.into());
        }

        query.push_str(" ORDER BY date DESC, total_events DESC");

        let stmt = Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            query,
            params,
        );
        let rows = db.query_all(stmt).await?;

        let mut activities = Vec::new();
        for row in rows {
            activities.push(UserActivity {
                user_id: row.try_get("", "user_id")?,
                date: row.try_get("", "date")?,
                total_events: row.try_get("", "total_events")?,
                event_types: row.try_get("", "event_types")?,
                first_event: row.try_get("", "first_event")?,
                last_event: row.try_get("", "last_event")?,
            });
        }

        Ok(activities)
    }

    async fn get_popular_events(
        &self, period: &str, limit: Option<i64>,
    ) -> Result<Vec<PopularEvents>, MaterializedViewError> {
        let db = self.db.get_connect();

        let mut query = "SELECT event_type, period, total_count, \
                         unique_users, growth_rate 
                        FROM popular_events 
                        WHERE period = $1 
                        ORDER BY total_count DESC"
            .to_string();

        let mut params: Vec<Value> = vec![period.into()];

        if let Some(l) = limit {
            query.push_str(" LIMIT $2");
            params.push(l.into());
        }

        let stmt = Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            query,
            params,
        );
        let rows = db.query_all(stmt).await?;

        let mut popular = Vec::new();
        for row in rows {
            popular.push(PopularEvents {
                event_type: row.try_get("", "event_type")?,
                period: row.try_get("", "period")?,
                total_count: row.try_get("", "total_count")?,
                unique_users: row.try_get("", "unique_users")?,
                growth_rate: row.try_get("", "growth_rate").ok(),
            });
        }

        Ok(popular)
    }
}
