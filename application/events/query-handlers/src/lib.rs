use std::{
    hash::{DefaultHasher, Hash, Hasher},
    time::Duration,
};

use database_traits::dao::GenericDao;
use events_cache_keys::{
    EventCacheKey, EventListCacheKey, UserEventsCacheKey,
    UserEventsLimitCacheKey,
};
use events_dao::EventDao;
use events_errors::EventError;
use events_models::Event;
use events_queries::{GetEventQuery, GetUserEventsQuery, ListEventsQuery};
use redis_connection::{
    cache_provider::CacheProvider,
    core::{CacheTypeBind, Json},
};
use sql_connection::SqlConnect;
use tracing::instrument;

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
    ) -> Result<Event, EventError> {
        let backend = CacheProvider::get_backend();

        // Try to get from cache first
        let cache_key = EventCacheKey;
        let mut cache = cache_key.bind_with(backend.clone(), &query.event_id);

        if let Ok(Some(event)) = cache.try_get().await {
            tracing::debug!("Cache hit for event {}", query.event_id);
            return Ok(event);
        }

        tracing::debug!(
            "Cache miss for event {}, fetching from DB",
            query.event_id
        );

        let event = self.event_dao.find_by_id(query.event_id).await.map_err(
            |_| {
                EventError::NotFound {
                    event_id: query.event_id,
                }
            },
        )?;

        // Cache for only 30 seconds - events are updated frequently
        let _ = cache
            .set_with_expire::<()>(
                Json(event.clone()),
                Duration::from_secs(30),
            )
            .await;

        Ok(event)
    }
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
    ) -> Result<Vec<Event>, EventError> {
        // Create a hash of the query parameters for cache key
        let mut hasher = DefaultHasher::new();
        query.user_id.hash(&mut hasher);
        query.event_type_id.hash(&mut hasher);
        query.limit.hash(&mut hasher);
        query.offset.hash(&mut hasher);
        let filter_hash = hasher.finish().to_string();

        let backend = CacheProvider::get_backend();

        // Try to get from cache first
        let cache_key = EventListCacheKey;
        let mut cache = cache_key.bind_with(backend.clone(), &filter_hash);

        if let Ok(Some(events)) = cache.try_get().await {
            tracing::debug!(
                "Cache hit for events list with filter {}",
                filter_hash
            );
            return Ok(events);
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
                Json(events.clone()),
                Duration::from_secs(15),
            )
            .await;

        Ok(events)
    }
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
    ) -> Result<Vec<Event>, EventError> {
        let backend = CacheProvider::get_backend();

        // Use different cache keys based on whether limit is specified
        if let Some(limit) = query.limit {
            let cache_key = UserEventsLimitCacheKey;
            let mut cache = cache_key
                .bind_with_args(backend.clone(), (&query.user_id, &limit));

            if let Ok(Some(events)) = cache.try_get().await {
                tracing::debug!(
                    "Cache hit for user {} events with limit {}",
                    query.user_id,
                    limit
                );
                return Ok(events);
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
                    Json(events.clone()),
                    Duration::from_secs(30),
                )
                .await;

            Ok(events)
        }
        else {
            let cache_key = UserEventsCacheKey;
            let mut cache =
                cache_key.bind_with(backend.clone(), &query.user_id);

            if let Ok(Some(events)) = cache.try_get().await {
                tracing::debug!(
                    "Cache hit for user {} events",
                    query.user_id
                );
                return Ok(events);
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
                    Json(events.clone()),
                    Duration::from_secs(30),
                )
                .await;

            Ok(events)
        }
    }
}

#[cfg(test)]
mod tests {
    use redis_connection::cache_provider::CacheProvider;
    use test_utils::{TestRedisContainer, *};

    use super::*;

    async fn setup_test_db() -> anyhow::Result<(
        test_utils::TestPostgresContainer,
        GetUserEventsQueryHandler,
    )> {
        let container = test_utils::TestPostgresContainer::new().await?;
        let redis_container = TestRedisContainer::new().await.unwrap();
        redis_container.flush_db().await.unwrap();

        // Initialize the cache provider with the Redis pool
        CacheProvider::init_redis_static(redis_container.pool.clone());

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
        let non_existent_user_id = 999999;

        let query = GetUserEventsQuery {
            user_id: non_existent_user_id,
            limit: None,
        };
        let result = handler.execute(query).await.unwrap();

        assert_eq!(result.len(), 0);
    }
}
