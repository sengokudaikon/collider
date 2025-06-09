use std::time::Duration;

use database_traits::dao::GenericDao;
use redis_connection::{
    connection::RedisConnectionManager, json::Json, type_bind::RedisTypeBind,
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
    ) -> Result<users::Model, GetUserError> {
        let redis = RedisConnectionManager::from_static();
        let mut conn = redis.get_connection().await?;

        // Try to get from cache first
        let cache_key = UserCacheKey;
        let mut cache = cache_key.bind_with(&mut *conn, &query.user_id);

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
                Json(user.clone()).serde().unwrap(),
                Duration::from_secs(300),
            )
            .await;

        Ok(user)
    }
}
