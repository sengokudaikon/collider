use std::time::Duration;

use events_dao::EventDao;
use events_models::EventModel;
use redis_connection::{
    connection::RedisConnectionManager, json::Json, type_bind::RedisTypeBind,
};
use serde::Deserialize;
use sql_connection::SqlConnect;
use thiserror::Error;
use tracing::instrument;
use uuid::Uuid;

use crate::cache_keys::{UserEventsCacheKey, UserEventsLimitCacheKey};

#[derive(Debug, Error)]
pub enum GetUserEventsError {
    #[error("DAO error: {0}")]
    Dao(#[from] events_dao::EventDaoError),
    #[error("Redis error: {0}")]
    Redis(#[from] redis_connection::RedisError),
    #[error("Redis pool error: {0}")]
    Pool(#[from] redis_connection::PoolError),
}

#[derive(Debug, Deserialize)]
pub struct GetUserEventsQuery {
    pub user_id: Uuid,
    pub limit: Option<u64>,
}

#[derive(Clone)]
pub struct GetUserEventsQueryHandler {
    event_dao: EventDao,
}

impl GetUserEventsQueryHandler {
    pub fn new(db: SqlConnect) -> Self {
        Self {
            event_dao: EventDao::new(db),
        }
    }

    #[instrument(skip(self))]
    pub async fn execute(
        &self, query: GetUserEventsQuery,
    ) -> Result<Vec<EventModel>, GetUserEventsError> {
        let redis = RedisConnectionManager::from_static();
        let mut conn = redis.get_connection().await?;

        // Use different cache keys based on whether limit is specified
        if let Some(limit) = query.limit {
            let cache_key = UserEventsLimitCacheKey;
            let mut cache = cache_key
                .bind_with_args(&mut *conn, (&query.user_id, &limit));

            if let Ok(Some(events)) = cache.try_get().await {
                tracing::debug!(
                    "Cache hit for user {} events with limit {}",
                    query.user_id,
                    limit
                );
                return Ok(events.inner());
            }

            tracing::debug!(
                "Cache miss for user {} events with limit {}, fetching from \
                 DB",
                query.user_id,
                limit
            );

            let events = self
                .event_dao
                .find_by_user_id(query.user_id, query.limit)
                .await?;

            // Cache for 30 seconds - user events change frequently
            let _ = cache
                .set_with_expire::<()>(
                    Json(events.clone()).serde().unwrap(),
                    Duration::from_secs(30),
                )
                .await;

            Ok(events)
        }
        else {
            let cache_key = UserEventsCacheKey;
            let mut cache = cache_key.bind_with(&mut *conn, &query.user_id);

            if let Ok(Some(events)) = cache.try_get().await {
                tracing::debug!(
                    "Cache hit for user {} events",
                    query.user_id
                );
                return Ok(events.inner());
            }

            tracing::debug!(
                "Cache miss for user {} events, fetching from DB",
                query.user_id
            );

            let events = self
                .event_dao
                .find_by_user_id(query.user_id, query.limit)
                .await?;

            // Cache for 30 seconds - user events change frequently
            let _ = cache
                .set_with_expire::<()>(
                    Json(events.clone()).serde().unwrap(),
                    Duration::from_secs(30),
                )
                .await;

            Ok(events)
        }
    }
}
