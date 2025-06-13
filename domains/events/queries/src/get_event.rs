use std::time::Duration;

use database_traits::dao::GenericDao;
use events_dao::EventDao;
use events_models::Event;
use redis_connection::{
    connection::RedisConnectionManager, json::Json, type_bind::RedisTypeBind,
};
use serde::Deserialize;
use sql_connection::SqlConnect;
use thiserror::Error;
use tracing::instrument;
use uuid::Uuid;

use crate::cache_keys::EventCacheKey;

#[derive(Debug, Error)]
pub enum GetEventError {
    #[error("DAO error: {0}")]
    Dao(#[from] events_dao::EventDaoError),
    #[error("Event not found: {event_id}")]
    NotFound { event_id: Uuid },
    #[error("Redis error: {0}")]
    Redis(#[from] redis_connection::RedisError),
    #[error("Redis pool error: {0}")]
    Pool(#[from] redis_connection::PoolError),
}

#[derive(Debug, Deserialize)]
pub struct GetEventQuery {
    pub event_id: Uuid,
}

#[derive(Clone)]
pub struct GetEventQueryHandler {
    event_dao: EventDao,
}

impl GetEventQueryHandler {
    pub fn new(db: SqlConnect) -> Self {
        Self {
            event_dao: EventDao::new(db),
        }
    }

    #[instrument(skip(self))]
    pub async fn execute(
        &self, query: GetEventQuery,
    ) -> Result<Event, GetEventError> {
        let redis = RedisConnectionManager::from_static();
        let mut conn = redis.get_connection().await?;

        // Try to get from cache first
        let cache_key = EventCacheKey;
        let mut cache = cache_key.bind_with(&mut *conn, &query.event_id);

        if let Ok(Some(event)) = cache.try_get().await {
            tracing::debug!("Cache hit for event {}", query.event_id);
            return Ok(event.inner());
        }

        tracing::debug!(
            "Cache miss for event {}, fetching from DB",
            query.event_id
        );

        let event = self.event_dao.find_by_id(query.event_id).await.map_err(
            |_| {
                GetEventError::NotFound {
                    event_id: query.event_id,
                }
            },
        )?;

        // Cache for only 30 seconds - events are updated frequently
        let _ = cache
            .set_with_expire::<()>(
                Json(event.clone()).serde().unwrap(),
                Duration::from_secs(30),
            )
            .await;

        Ok(event)
    }
}
