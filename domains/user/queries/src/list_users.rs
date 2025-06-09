use std::time::Duration;

use redis_connection::{
    connection::RedisConnectionManager, json::Json, type_bind::RedisTypeBind,
};
use serde::Deserialize;
use sql_connection::SqlConnect;
use thiserror::Error;
use tracing::instrument;
use user_dao::UserDao;
use user_models as users;

use crate::cache_keys::UserListCacheKey;

#[derive(Debug, Error)]
pub enum ListUsersError {
    #[error("DAO error: {0}")]
    Dao(#[from] user_dao::UserDaoError),
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("Redis pool error: {0}")]
    Pool(#[from] redis_connection::PoolError),
}

#[derive(Debug, Deserialize)]
pub struct ListUsersQuery {
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

#[derive(Clone)]
pub struct ListUsersQueryHandler {
    user_dao: UserDao,
}

impl ListUsersQueryHandler {
    pub fn new(db: SqlConnect) -> Self {
        Self {
            user_dao: UserDao::new(db),
        }
    }

    #[instrument(skip(self))]
    pub async fn execute(
        &self, query: ListUsersQuery,
    ) -> Result<Vec<users::Model>, ListUsersError> {
        // For paginated queries, we'll cache with a specific key including
        // limit/offset For now, we'll only cache the default list (no
        // pagination)
        if query.limit.is_none() && query.offset.is_none() {
            let redis = RedisConnectionManager::from_static();
            let mut conn = redis.get_connection().await?;

            let cache_key = UserListCacheKey;
            let mut cache = cache_key.bind(&mut *conn);

            if let Ok(Some(users)) = cache.try_get().await {
                tracing::debug!("Cache hit for user list");
                return Ok(users.inner());
            }

            tracing::debug!("Cache miss for user list, fetching from DB");

            let users =
                self.user_dao.find_with_pagination(None, None).await?;

            // Cache for 2 minutes - list might change more often than
            // individual users
            let _ = cache
                .set_with_expire::<()>(
                    Json(users.clone()).serde().unwrap(),
                    Duration::from_secs(120),
                )
                .await;

            Ok(users)
        }
        else {
            // For paginated queries, skip cache for now
            let users = self
                .user_dao
                .find_with_pagination(query.limit, query.offset)
                .await?;
            Ok(users)
        }
    }
}
