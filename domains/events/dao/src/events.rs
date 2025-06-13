use async_trait::async_trait;
use database_traits::dao::GenericDao;
use events_models::{Event, NewEvent, UpdateEvent};
use sql_connection::SqlConnect;
use thiserror::Error;
use tracing::instrument;
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Error)]
pub enum EventDaoError {
    #[error("Database error: {0}")]
    Database(#[from] tokio_postgres::Error),
    #[error("Connection error: {0}")]
    Connection(#[from] deadpool_postgres::PoolError),
    #[error("Event not found")]
    NotFound,
}

#[derive(Clone)]
pub struct EventDao {
    db: SqlConnect,
}

impl EventDao {
    pub fn new(db: SqlConnect) -> Self { Self { db } }
}

#[async_trait]
impl GenericDao for EventDao {
    type Model = Event;
    type Response = Event;
    type CreateRequest = NewEvent;
    type UpdateRequest = UpdateEvent;
    type Error = EventDaoError;
    type ID = Uuid;

    async fn find_by_id(
        &self, id: Self::ID,
    ) -> Result<Self::Response, Self::Error> {
        let client = self.db.get_client().await?;
        let stmt = client.prepare(
            "SELECT id, user_id, event_type_id, timestamp, metadata 
             FROM events WHERE id = $1"
        ).await?;
        let rows = client.query(&stmt, &[&id]).await?;
        
        let event = rows.first()
            .map(|row| Event {
                id: row.get(0),
                user_id: row.get(1),
                event_type_id: row.get(2),
                timestamp: row.get(3),
                metadata: row.get(4),
            })
            .ok_or(EventDaoError::NotFound)?;
        
        Ok(event)
    }

    async fn all(&self) -> Result<Vec<Self::Response>, Self::Error> {
        let client = self.db.get_client().await?;
        let stmt = client.prepare(
            "SELECT id, user_id, event_type_id, timestamp, metadata 
             FROM events ORDER BY timestamp DESC"
        ).await?;
        let rows = client.query(&stmt, &[]).await?;
        
        let events = rows.iter()
            .map(|row| Event {
                id: row.get(0),
                user_id: row.get(1),
                event_type_id: row.get(2),
                timestamp: row.get(3),
                metadata: row.get(4),
            })
            .collect();
        
        Ok(events)
    }

    async fn create(
        &self, req: Self::CreateRequest,
    ) -> Result<Self::Response, Self::Error> {
        let client = self.db.get_client().await?;
        
        let stmt = client.prepare(
            "INSERT INTO events (id, user_id, event_type_id, timestamp, metadata) 
             VALUES ($1, $2, $3, $4, $5) 
             RETURNING id, user_id, event_type_id, timestamp, metadata"
        ).await?;
        
        let rows = client.query(&stmt, &[
            &req.id,
            &req.user_id,
            &req.event_type_id,
            &req.timestamp,
            &req.metadata,
        ]).await?;
        
        let event = rows.first()
            .map(|row| Event {
                id: row.get(0),
                user_id: row.get(1),
                event_type_id: row.get(2),
                timestamp: row.get(3),
                metadata: row.get(4),
            })
            .ok_or_else(|| EventDaoError::Database(
                tokio_postgres::Error::__private_api_timeout()
            ))?;
        
        Ok(event)
    }

    async fn update(
        &self, id: Self::ID, req: Self::UpdateRequest,
    ) -> Result<Self::Response, Self::Error> {
        let client = self.db.get_client().await?;
        
        // Check if event exists first
        let check_stmt = client.prepare("SELECT id FROM events WHERE id = $1").await?;
        let check_rows = client.query(&check_stmt, &[&id]).await?;
        if check_rows.is_empty() {
            return Err(EventDaoError::NotFound);
        }
        
        // Build update query dynamically based on provided fields
        let mut updates = Vec::new();
        let mut params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = vec![&id];
        let mut param_count = 1;
        
        if let Some(user_id) = &req.user_id {
            param_count += 1;
            updates.push(format!("user_id = ${}", param_count));
            params.push(user_id);
        }
        
        if let Some(event_type_id) = &req.event_type_id {
            param_count += 1;
            updates.push(format!("event_type_id = ${}", param_count));
            params.push(event_type_id);
        }
        
        if let Some(metadata) = &req.metadata {
            param_count += 1;
            updates.push(format!("metadata = ${}", param_count));
            params.push(metadata);
        }
        
        if updates.is_empty() {
            // No updates, just return the existing event
            return self.find_by_id(id).await;
        }
        
        let query = format!(
            "UPDATE events SET {} WHERE id = $1 
             RETURNING id, user_id, event_type_id, timestamp, metadata",
            updates.join(", ")
        );
        
        let stmt = client.prepare(&query).await?;
        let rows = client.query(&stmt, &params).await?;
        
        let event = rows.first()
            .map(|row| Event {
                id: row.get(0),
                user_id: row.get(1),
                event_type_id: row.get(2),
                timestamp: row.get(3),
                metadata: row.get(4),
            })
            .ok_or(EventDaoError::NotFound)?;
        
        Ok(event)
    }

    async fn delete(&self, id: Self::ID) -> Result<(), Self::Error> {
        let client = self.db.get_client().await?;
        let stmt = client.prepare("DELETE FROM events WHERE id = $1").await?;
        let affected = client.execute(&stmt, &[&id]).await?;
        
        if affected == 0 {
            return Err(EventDaoError::NotFound);
        }
        
        Ok(())
    }
}

impl EventDao {
    #[instrument(skip_all)]
    pub async fn find_with_filters(
        &self, user_id: Option<Uuid>, event_type_id: Option<i32>,
        limit: Option<u64>, offset: Option<u64>,
    ) -> Result<Vec<Event>, EventDaoError> {
        let client = self.db.get_client().await?;
        
        // Handle different combinations of filters to avoid lifetime issues
        let rows = match (user_id, event_type_id, limit, offset) {
            (Some(uid), Some(etid), Some(l), Some(o)) => {
                let query = "SELECT id, user_id, event_type_id, timestamp, metadata FROM events 
                           WHERE user_id = $1 AND event_type_id = $2 
                           ORDER BY timestamp DESC LIMIT $3 OFFSET $4";
                let stmt = client.prepare(query).await?;
                client.query(&stmt, &[&uid, &etid, &(l as i64), &(o as i64)]).await?
            },
            (Some(uid), Some(etid), Some(l), None) => {
                let query = "SELECT id, user_id, event_type_id, timestamp, metadata FROM events 
                           WHERE user_id = $1 AND event_type_id = $2 
                           ORDER BY timestamp DESC LIMIT $3";
                let stmt = client.prepare(query).await?;
                client.query(&stmt, &[&uid, &etid, &(l as i64)]).await?
            },
            (Some(uid), Some(etid), None, Some(o)) => {
                let query = "SELECT id, user_id, event_type_id, timestamp, metadata FROM events 
                           WHERE user_id = $1 AND event_type_id = $2 
                           ORDER BY timestamp DESC OFFSET $3";
                let stmt = client.prepare(query).await?;
                client.query(&stmt, &[&uid, &etid, &(o as i64)]).await?
            },
            (Some(uid), Some(etid), None, None) => {
                let query = "SELECT id, user_id, event_type_id, timestamp, metadata FROM events 
                           WHERE user_id = $1 AND event_type_id = $2 
                           ORDER BY timestamp DESC";
                let stmt = client.prepare(query).await?;
                client.query(&stmt, &[&uid, &etid]).await?
            },
            (Some(uid), None, Some(l), Some(o)) => {
                let query = "SELECT id, user_id, event_type_id, timestamp, metadata FROM events 
                           WHERE user_id = $1 
                           ORDER BY timestamp DESC LIMIT $2 OFFSET $3";
                let stmt = client.prepare(query).await?;
                client.query(&stmt, &[&uid, &(l as i64), &(o as i64)]).await?
            },
            (Some(uid), None, Some(l), None) => {
                let query = "SELECT id, user_id, event_type_id, timestamp, metadata FROM events 
                           WHERE user_id = $1 
                           ORDER BY timestamp DESC LIMIT $2";
                let stmt = client.prepare(query).await?;
                client.query(&stmt, &[&uid, &(l as i64)]).await?
            },
            (Some(uid), None, None, Some(o)) => {
                let query = "SELECT id, user_id, event_type_id, timestamp, metadata FROM events 
                           WHERE user_id = $1 
                           ORDER BY timestamp DESC OFFSET $2";
                let stmt = client.prepare(query).await?;
                client.query(&stmt, &[&uid, &(o as i64)]).await?
            },
            (Some(uid), None, None, None) => {
                let query = "SELECT id, user_id, event_type_id, timestamp, metadata FROM events 
                           WHERE user_id = $1 
                           ORDER BY timestamp DESC";
                let stmt = client.prepare(query).await?;
                client.query(&stmt, &[&uid]).await?
            },
            (None, Some(etid), Some(l), Some(o)) => {
                let query = "SELECT id, user_id, event_type_id, timestamp, metadata FROM events 
                           WHERE event_type_id = $1 
                           ORDER BY timestamp DESC LIMIT $2 OFFSET $3";
                let stmt = client.prepare(query).await?;
                client.query(&stmt, &[&etid, &(l as i64), &(o as i64)]).await?
            },
            (None, Some(etid), Some(l), None) => {
                let query = "SELECT id, user_id, event_type_id, timestamp, metadata FROM events 
                           WHERE event_type_id = $1 
                           ORDER BY timestamp DESC LIMIT $2";
                let stmt = client.prepare(query).await?;
                client.query(&stmt, &[&etid, &(l as i64)]).await?
            },
            (None, Some(etid), None, Some(o)) => {
                let query = "SELECT id, user_id, event_type_id, timestamp, metadata FROM events 
                           WHERE event_type_id = $1 
                           ORDER BY timestamp DESC OFFSET $2";
                let stmt = client.prepare(query).await?;
                client.query(&stmt, &[&etid, &(o as i64)]).await?
            },
            (None, Some(etid), None, None) => {
                let query = "SELECT id, user_id, event_type_id, timestamp, metadata FROM events 
                           WHERE event_type_id = $1 
                           ORDER BY timestamp DESC";
                let stmt = client.prepare(query).await?;
                client.query(&stmt, &[&etid]).await?
            },
            (None, None, Some(l), Some(o)) => {
                let query = "SELECT id, user_id, event_type_id, timestamp, metadata FROM events 
                           ORDER BY timestamp DESC LIMIT $1 OFFSET $2";
                let stmt = client.prepare(query).await?;
                client.query(&stmt, &[&(l as i64), &(o as i64)]).await?
            },
            (None, None, Some(l), None) => {
                let query = "SELECT id, user_id, event_type_id, timestamp, metadata FROM events 
                           ORDER BY timestamp DESC LIMIT $1";
                let stmt = client.prepare(query).await?;
                client.query(&stmt, &[&(l as i64)]).await?
            },
            (None, None, None, Some(o)) => {
                let query = "SELECT id, user_id, event_type_id, timestamp, metadata FROM events 
                           ORDER BY timestamp DESC OFFSET $1";
                let stmt = client.prepare(query).await?;
                client.query(&stmt, &[&(o as i64)]).await?
            },
            (None, None, None, None) => {
                let query = "SELECT id, user_id, event_type_id, timestamp, metadata FROM events 
                           ORDER BY timestamp DESC";
                let stmt = client.prepare(query).await?;
                client.query(&stmt, &[]).await?
            },
        };
        
        let events = rows.iter()
            .map(|row| Event {
                id: row.get(0),
                user_id: row.get(1),
                event_type_id: row.get(2),
                timestamp: row.get(3),
                metadata: row.get(4),
            })
            .collect();
        
        Ok(events)
    }

    #[instrument(skip_all)]
    pub async fn delete_before_timestamp(
        &self, before: DateTime<Utc>,
    ) -> Result<u64, EventDaoError> {
        let client = self.db.get_client().await?;
        let stmt = client.prepare("DELETE FROM events WHERE timestamp < $1").await?;
        let affected = client.execute(&stmt, &[&before]).await?;
        Ok(affected)
    }

    #[instrument(skip_all)]
    pub async fn find_by_user_id(
        &self, user_id: Uuid, limit: Option<u64>,
    ) -> Result<Vec<Event>, EventDaoError> {
        let client = self.db.get_client().await?;
        
        let (query, limit_i64) = if let Some(limit) = limit {
            (
                "SELECT id, user_id, event_type_id, timestamp, metadata 
                 FROM events WHERE user_id = $1 
                 ORDER BY timestamp DESC LIMIT $2",
                Some(limit as i64)
            )
        } else {
            (
                "SELECT id, user_id, event_type_id, timestamp, metadata 
                 FROM events WHERE user_id = $1 
                 ORDER BY timestamp DESC",
                None
            )
        };
        
        let stmt = client.prepare(query).await?;
        let rows = if let Some(limit) = limit_i64 {
            client.query(&stmt, &[&user_id, &limit]).await?
        } else {
            client.query(&stmt, &[&user_id]).await?
        };
        
        let events = rows.iter()
            .map(|row| Event {
                id: row.get(0),
                user_id: row.get(1),
                event_type_id: row.get(2),
                timestamp: row.get(3),
                metadata: row.get(4),
            })
            .collect();
        
        Ok(events)
    }
}