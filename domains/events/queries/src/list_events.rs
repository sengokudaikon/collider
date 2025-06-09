use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    time::Duration,
};

use events_dao::EventDao;
use events_models::EventResponse;
use redis_connection::{
    connection::RedisConnectionManager, json::Json, type_bind::RedisTypeBind,
};
use serde::Deserialize;
use sql_connection::SqlConnect;
use thiserror::Error;
use tracing::instrument;
use uuid::Uuid;

use crate::cache_keys::EventListCacheKey;

#[derive(Debug, Error)]
pub enum ListEventsError {
    #[error("DAO error: {0}")]
    Dao(#[from] events_dao::EventDaoError),
    #[error("Redis error: {0}")]
    Redis(#[from] redis_connection::RedisError),
    #[error("Redis pool error: {0}")]
    Pool(#[from] redis_connection::PoolError),
}

#[derive(Debug, Deserialize)]
pub struct ListEventsQuery {
    pub user_id: Option<Uuid>,
    pub event_type_id: Option<i32>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

#[derive(Clone)]
pub struct ListEventsQueryHandler {
    event_dao: EventDao,
}

impl ListEventsQueryHandler {
    pub fn new(db: SqlConnect) -> Self {
        Self {
            event_dao: EventDao::new(db),
        }
    }

    #[instrument(skip(self))]
    pub async fn execute(
        &self, query: ListEventsQuery,
    ) -> Result<Vec<EventResponse>, ListEventsError> {
        // Create a hash of the query parameters for cache key
        let mut hasher = DefaultHasher::new();
        query.user_id.hash(&mut hasher);
        query.event_type_id.hash(&mut hasher);
        query.limit.hash(&mut hasher);
        query.offset.hash(&mut hasher);
        let filter_hash = hasher.finish().to_string();

        let redis = RedisConnectionManager::from_static();
        let mut conn = redis.get_connection().await?;

        // Try to get from cache first
        let cache_key = EventListCacheKey;
        let mut cache = cache_key.bind_with(&mut *conn, &filter_hash);

        if let Ok(Some(events)) = cache.try_get().await {
            tracing::debug!(
                "Cache hit for events list with filter {}",
                filter_hash
            );
            return Ok(events.inner());
        }

        tracing::debug!(
            "Cache miss for events list with filter {}, fetching from DB",
            filter_hash
        );

        let events = self
            .event_dao
            .find_with_filters(
                query.user_id,
                query.event_type_id,
                query.limit,
                query.offset,
            )
            .await?;

        // Cache for only 15 seconds - event lists change frequently
        let _ = cache
            .set_with_expire::<()>(
                Json(events.clone()).serde().unwrap(),
                Duration::from_secs(15),
            )
            .await;

        Ok(events)
    }
}
