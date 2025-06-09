use std::time::Duration;

use redis_connection::{
    connection::RedisConnectionManager, json::Json, type_bind::RedisTypeBind,
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

impl From<users::Model> for GetUserByNameResponse {
    fn from(user: users::Model) -> Self {
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
        let mut cache = cache_key.bind_with(&mut *conn, &query.name);

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
                Json(user.clone()).serde().unwrap(),
                Duration::from_secs(300),
            )
            .await;

        Ok(user.into())
    }
}
