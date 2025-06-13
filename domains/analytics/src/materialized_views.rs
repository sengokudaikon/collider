use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sql_connection::SqlConnect;
use thiserror::Error;
use tokio_postgres::Error as PgError;
use tracing::{info, instrument};
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum MaterializedViewError {
    #[error("Database error: {0}")]
    Database(#[from] PgError),
    #[error("Connection error: {0}")]
    Connection(#[from] deadpool_postgres::PoolError),
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
        let client = self.db.get_client().await?;
        client.execute(
            "REFRESH MATERIALIZED VIEW CONCURRENTLY event_hourly_summaries",
            &[],
        )
        .await?;

        info!("Refreshed hourly summaries materialized view");
        Ok(())
    }

    #[instrument(skip(self))]
    async fn refresh_daily_user_activity(
        &self,
    ) -> Result<(), MaterializedViewError> {
        let client = self.db.get_client().await?;
        client.execute(
            "REFRESH MATERIALIZED VIEW CONCURRENTLY user_daily_activity",
            &[],
        )
        .await?;

        info!("Refreshed daily user activity materialized view");
        Ok(())
    }

    #[instrument(skip(self))]
    async fn refresh_popular_events(
        &self,
    ) -> Result<(), MaterializedViewError> {
        let client = self.db.get_client().await?;
        client.execute(
            "REFRESH MATERIALIZED VIEW CONCURRENTLY popular_events",
            &[],
        )
        .await?;

        info!("Refreshed popular events materialized view");
        Ok(())
    }

    async fn get_hourly_summaries(
        &self, start: DateTime<Utc>, end: DateTime<Utc>,
        event_types: Option<Vec<String>>,
    ) -> Result<Vec<EventSummary>, MaterializedViewError> {
        let client = self.db.get_client().await?;

        let (_query, rows) = if let Some(types) = event_types {
            let query = "SELECT event_type, hour, total_events, unique_users, avg_events_per_user 
                        FROM event_hourly_summaries 
                        WHERE hour >= $1 AND hour <= $2 AND event_type = ANY($3)
                        ORDER BY hour DESC, total_events DESC";
            let stmt = client.prepare(query).await?;
            let rows = client.query(&stmt, &[&start, &end, &types]).await?;
            (query, rows)
        } else {
            let query = "SELECT event_type, hour, total_events, unique_users, avg_events_per_user 
                        FROM event_hourly_summaries 
                        WHERE hour >= $1 AND hour <= $2
                        ORDER BY hour DESC, total_events DESC";
            let stmt = client.prepare(query).await?;
            let rows = client.query(&stmt, &[&start, &end]).await?;
            (query, rows)
        };

        let mut summaries = Vec::new();
        for row in rows {
            summaries.push(EventSummary {
                event_type: row.get(0),
                hour: row.get(1),
                total_events: row.get(2),
                unique_users: row.get(3),
                avg_events_per_user: row.get(4),
            });
        }

        Ok(summaries)
    }

    async fn get_user_activity(
        &self, user_id: Option<Uuid>, start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<UserActivity>, MaterializedViewError> {
        let client = self.db.get_client().await?;

        let rows = if let Some(uid) = user_id {
            let query = "SELECT user_id, date, total_events, event_types, first_event, last_event 
                        FROM user_daily_activity 
                        WHERE date >= $1 AND date <= $2 AND user_id = $3
                        ORDER BY date DESC, total_events DESC";
            let stmt = client.prepare(query).await?;
            client.query(&stmt, &[&start, &end, &uid]).await?
        } else {
            let query = "SELECT user_id, date, total_events, event_types, first_event, last_event 
                        FROM user_daily_activity 
                        WHERE date >= $1 AND date <= $2
                        ORDER BY date DESC, total_events DESC";
            let stmt = client.prepare(query).await?;
            client.query(&stmt, &[&start, &end]).await?
        };

        let mut activities = Vec::new();
        for row in rows {
            activities.push(UserActivity {
                user_id: row.get(0),
                date: row.get(1),
                total_events: row.get(2),
                event_types: row.get(3),
                first_event: row.get(4),
                last_event: row.get(5),
            });
        }

        Ok(activities)
    }

    async fn get_popular_events(
        &self, period: &str, limit: Option<i64>,
    ) -> Result<Vec<PopularEvents>, MaterializedViewError> {
        let client = self.db.get_client().await?;

        let rows = if let Some(l) = limit {
            let query = "SELECT event_type, period, total_count, unique_users, growth_rate 
                        FROM popular_events 
                        WHERE period = $1 
                        ORDER BY total_count DESC LIMIT $2";
            let stmt = client.prepare(query).await?;
            client.query(&stmt, &[&period, &l]).await?
        } else {
            let query = "SELECT event_type, period, total_count, unique_users, growth_rate 
                        FROM popular_events 
                        WHERE period = $1 
                        ORDER BY total_count DESC";
            let stmt = client.prepare(query).await?;
            client.query(&stmt, &[&period]).await?
        };

        let mut popular = Vec::new();
        for row in rows {
            popular.push(PopularEvents {
                event_type: row.get(0),
                period: row.get(1),
                total_count: row.get(2),
                unique_users: row.get(3),
                growth_rate: row.try_get(4).ok(),
            });
        }

        Ok(popular)
    }
}
