use std::time::Duration;

use redis_connection::{
    connection::RedisConnectionManager,
    core::{CacheTypeBind, value::Json},
};
use serde::{Deserialize, Serialize};
use sql_connection::SqlConnect;
use thiserror::Error;
use tracing::instrument;
use user_dao::{UserDao, UserDaoError};
use user_models as users;

use crate::cache_keys::UserByNameCacheKey;

#[derive(Debug, Error)]
pub enum GetUserByNameError {
    #[error("DAO error: {0}")]
    Dao(#[from] UserDaoError),
    #[error("User not found with name: {0}")]
    NotFound(String),
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("Redis pool error: {0}")]
    Pool(#[from] redis_connection::PoolError),
}

#[derive(Debug, Deserialize)]
pub struct GetUserByNameQuery {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct GetUserByNameResponse {
    pub id: uuid::Uuid,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<users::User> for GetUserByNameResponse {
    fn from(user: users::User) -> Self {
        Self {
            id: user.id,
            name: user.name,
            created_at: user.created_at,
        }
    }
}

#[derive(Clone)]
pub struct GetUserByNameQueryHandler {
    user_dao: UserDao,
}

impl GetUserByNameQueryHandler {
    pub fn new(db: SqlConnect) -> Self {
        Self {
            user_dao: UserDao::new(db),
        }
    }

    #[instrument(skip(self))]
    pub async fn execute(
        &self, query: GetUserByNameQuery,
    ) -> Result<GetUserByNameResponse, GetUserByNameError> {
        let redis = RedisConnectionManager::from_static();
        let mut conn = redis.get_connection().await?;

        // Try to get from cache first
        let cache_key = UserByNameCacheKey;
        let mut cache = cache_key.bind_with(&mut conn, &query.name);

        if let Ok(Some(user)) = cache.try_get().await {
            tracing::debug!("Cache hit for user by name {}", query.name);
            return Ok(GetUserByNameResponse::from(user.inner()));
        }

        tracing::debug!(
            "Cache miss for user by name {}, fetching from DB",
            query.name
        );

        let user =
            self.user_dao.find_by_name(&query.name).await?.ok_or_else(
                || GetUserByNameError::NotFound(query.name.clone()),
            )?;

        // Cache for 5 minutes - user data doesn't change often
        let _ = cache
            .set_with_expire::<()>(
                Json(user.clone()),
                Duration::from_secs(300),
            )
            .await;

        Ok(user.into())
    }
}

#[cfg(test)]
mod tests {
    use test_utils::{redis::TestRedisContainer, *};

    use super::*;

    async fn setup_test_db() -> anyhow::Result<(
        test_utils::postgres::TestPostgresContainer,
        GetUserByNameQueryHandler,
    )> {
        let container =
            test_utils::postgres::TestPostgresContainer::new().await?;
        let redis_container = TestRedisContainer::new().await.unwrap();
        redis_container.flush_db().await?;
        let sql_connect = create_sql_connect(&container);
        let handler = GetUserByNameQueryHandler::new(sql_connect);
        Ok((container, handler))
    }

    #[tokio::test]
    async fn test_get_user_by_name_success() {
        let (container, handler) = setup_test_db().await.unwrap();
        let user_id = create_test_user_with_name(&container, "Alice")
            .await
            .unwrap();

        let query = GetUserByNameQuery {
            name: "Alice".to_string(),
        };
        let result = handler.execute(query).await.unwrap();

        assert_eq!(result.id, user_id);
        assert_eq!(result.name, "Alice");
    }

    #[tokio::test]
    async fn test_get_user_by_name_not_found() {
        let (_container, handler) = setup_test_db().await.unwrap();

        let query = GetUserByNameQuery {
            name: "NonExistentUser".to_string(),
        };
        let result = handler.execute(query).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            GetUserByNameError::NotFound(name) => {
                assert_eq!(name, "NonExistentUser");
            }
            _ => panic!("Expected NotFound error"),
        }
    }
}
