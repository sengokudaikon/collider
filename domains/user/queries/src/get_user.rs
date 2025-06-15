use std::time::Duration;

use database_traits::dao::GenericDao;
use redis_connection::{
    connection::RedisConnectionManager,
    core::{CacheTypeBind, value::Json},
};
use serde::Deserialize;
use sql_connection::SqlConnect;
use thiserror::Error;
use tracing::instrument;
use user_dao::UserDao;
use user_models as users;
use uuid::Uuid;

use crate::cache_keys::UserCacheKey;

#[derive(Debug, Error)]
pub enum GetUserError {
    #[error("DAO error: {0}")]
    Dao(#[from] user_dao::UserDaoError),
    #[error("User not found: {user_id}")]
    NotFound { user_id: Uuid },
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("Redis pool error: {0}")]
    Pool(#[from] redis_connection::PoolError),
}

#[derive(Debug, Deserialize)]
pub struct GetUserQuery {
    pub user_id: Uuid,
}

#[derive(Clone)]
pub struct GetUserQueryHandler {
    user_dao: UserDao,
}

impl GetUserQueryHandler {
    pub fn new(db: SqlConnect) -> Self {
        Self {
            user_dao: UserDao::new(db),
        }
    }

    #[instrument(skip(self))]
    pub async fn execute(
        &self, query: GetUserQuery,
    ) -> Result<users::User, GetUserError> {
        let redis = RedisConnectionManager::from_static();
        let mut conn = redis.get_connection().await?;

        // Try to get from cache first
        let cache_key = UserCacheKey;
        let mut cache = cache_key.bind_with(&mut conn, &query.user_id);

        if let Ok(Some(user)) = cache.try_get().await {
            tracing::debug!("Cache hit for user {}", query.user_id);
            return Ok(user.inner());
        }

        tracing::debug!(
            "Cache miss for user {}, fetching from DB",
            query.user_id
        );

        let user =
            self.user_dao.find_by_id(query.user_id).await.map_err(|_| {
                GetUserError::NotFound {
                    user_id: query.user_id,
                }
            })?;

        // Cache for 5 minutes - user data doesn't change often
        let _ = cache
            .set_with_expire::<()>(
                Json(user.clone()),
                Duration::from_secs(300),
            )
            .await;

        Ok(user)
    }
}

#[cfg(test)]
mod tests {
    use test_utils::{redis::TestRedisContainer, *};
    use uuid::Uuid;

    use super::*;

    async fn setup_test_db() -> anyhow::Result<(
        test_utils::postgres::TestPostgresContainer,
        GetUserQueryHandler,
    )> {
        let container =
            test_utils::postgres::TestPostgresContainer::new().await?;
        let redis_container = TestRedisContainer::new().await.unwrap();
        redis_container.flush_db().await.unwrap();
        let sql_connect = create_sql_connect(&container);
        let handler = GetUserQueryHandler::new(sql_connect);
        Ok((container, handler))
    }

    #[tokio::test]
    async fn test_get_user_success() {
        let (container, handler) = setup_test_db().await.unwrap();
        let user_id = create_test_user(&container).await.unwrap();

        let query = GetUserQuery { user_id };
        let result = handler.execute(query).await.unwrap();

        assert_eq!(result.id, user_id);
        assert_eq!(result.name, "Test User");
    }

    #[tokio::test]
    async fn test_get_user_not_found() {
        let (_container, handler) = setup_test_db().await.unwrap();
        let non_existent_user_id = Uuid::now_v7();

        let query = GetUserQuery {
            user_id: non_existent_user_id,
        };
        let result = handler.execute(query).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            GetUserError::NotFound { user_id } => {
                assert_eq!(user_id, non_existent_user_id);
            }
            _ => panic!("Expected NotFound error"),
        }
    }
}
