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

#[cfg(test)]
mod tests {
    use test_utils::{redis::TestRedisContainer, *};
    use uuid::Uuid;

    use super::*;

    async fn setup_test_db() -> anyhow::Result<(
        test_utils::postgres::TestPostgresContainer,
        GetUserEventsQueryHandler,
    )> {
        let container =
            test_utils::postgres::TestPostgresContainer::new().await?;
        let redis_container = TestRedisContainer::new().await.unwrap();
        redis_container.flush_db().await.unwrap();

        let sql_connect = create_sql_connect(&container);
        let handler = GetUserEventsQueryHandler::new(sql_connect);
        Ok((container, handler))
    }

    #[tokio::test]
    async fn test_get_user_events_without_limit() {
        let (container, handler) = setup_test_db().await.unwrap();
        let event_type_id = create_test_event_type(&container).await.unwrap();
        let user_id = create_test_user(&container).await.unwrap();
        let _event1_id =
            create_test_event(&container, user_id, event_type_id, None)
                .await
                .unwrap();
        let _event2_id = create_test_event(
            &container,
            user_id,
            event_type_id,
            Some(r#"{"key": "value"}"#),
        )
        .await
        .unwrap();

        let query = GetUserEventsQuery {
            user_id,
            limit: None,
        };
        let result = handler.execute(query).await.unwrap();

        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|e| e.user_id == user_id));
    }

    #[tokio::test]
    async fn test_get_user_events_with_limit() {
        let (container, handler) = setup_test_db().await.unwrap();
        let event_type_id = create_test_event_type(&container).await.unwrap();
        let user_id = create_test_user(&container).await.unwrap();
        let _event1_id =
            create_test_event(&container, user_id, event_type_id, None)
                .await
                .unwrap();
        let _event2_id =
            create_test_event(&container, user_id, event_type_id, None)
                .await
                .unwrap();
        let _event3_id =
            create_test_event(&container, user_id, event_type_id, None)
                .await
                .unwrap();

        let query = GetUserEventsQuery {
            user_id,
            limit: Some(2),
        };
        let result = handler.execute(query).await.unwrap();

        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|e| e.user_id == user_id));
    }

    #[tokio::test]
    async fn test_get_user_events_empty() {
        let (container, handler) = setup_test_db().await.unwrap();
        let user_id = create_test_user(&container).await.unwrap();

        let query = GetUserEventsQuery {
            user_id,
            limit: None,
        };
        let result = handler.execute(query).await.unwrap();

        assert_eq!(result.len(), 0);
    }

    #[tokio::test]
    async fn test_get_user_events_non_existent_user() {
        let (_container, handler) = setup_test_db().await.unwrap();
        let non_existent_user_id = Uuid::now_v7();

        let query = GetUserEventsQuery {
            user_id: non_existent_user_id,
            limit: None,
        };
        let result = handler.execute(query).await.unwrap();

        assert_eq!(result.len(), 0);
    }
}
