use std::time::Duration;

use database_traits::dao::GenericDao;
use redis_connection::{cache_provider::CacheProvider, core::CacheTypeBind};
use sql_connection::SqlConnect;
use tracing::instrument;
use user_cache_keys::{UserByNameCacheKey, UserCacheKey, UserListCacheKey};
use user_dao::UserDao;
use user_errors::UserError;
use user_queries::{GetUserByNameQuery, GetUserQuery, ListUsersQuery};
use user_responses::UserResponse;

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
    ) -> Result<user_models::User, UserError> {
        let backend = CacheProvider::get_backend();

        // Try to get from cache first
        let cache_key = UserCacheKey;
        let mut cache = cache_key.bind_with(backend.clone(), &query.user_id);

        if let Ok(Some(user)) = cache.try_get().await {
            tracing::debug!("Cache hit for user {}", query.user_id);
            return Ok(user);
        }

        tracing::debug!(
            "Cache miss for user {}, fetching from DB",
            query.user_id
        );

        let user =
            self.user_dao.find_by_id(query.user_id).await.map_err(|_| {
                UserError::NotFound {
                    user_id: query.user_id,
                }
            })?;

        // Cache for 5 minutes - user data doesn't change often
        let _ = cache
            .set_with_expire::<()>(user.clone(), Duration::from_secs(300))
            .await;

        Ok(user)
    }
}

#[cfg(test)]
mod tests {
    use redis_connection::cache_provider::CacheProvider;
    use test_utils::{TestRedisContainer, *};
    use user_queries::{GetUserByNameQuery, GetUserQuery, ListUsersQuery};

    use super::*;

    async fn setup_test_db()
    -> anyhow::Result<(TestPostgresContainer, GetUserQueryHandler)> {
        let container = TestPostgresContainer::new().await?;
        let redis_container = TestRedisContainer::new().await?;
        redis_container.flush_db().await?;

        // Initialize the cache provider with the Redis pool
        CacheProvider::init_redis_static(redis_container.pool.clone());

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
        let non_existent_user_id = 999999;

        let query = GetUserQuery {
            user_id: non_existent_user_id,
        };
        let result = handler.execute(query).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            UserError::NotFound { user_id } => {
                assert_eq!(user_id, non_existent_user_id);
            }
            _ => panic!("Expected NotFound error"),
        }
    }

    async fn setup_test_db_for_name_queries()
    -> anyhow::Result<(TestPostgresContainer, GetUserByNameQueryHandler)>
    {
        let container = TestPostgresContainer::new().await?;
        let redis_container = TestRedisContainer::new().await?;
        redis_container.flush_db().await?;

        // Initialize the cache provider with the Redis pool
        CacheProvider::init_redis_static(redis_container.pool.clone());

        let sql_connect = create_sql_connect(&container);
        let handler = GetUserByNameQueryHandler::new(sql_connect);
        Ok((container, handler))
    }

    #[tokio::test]
    async fn test_get_user_by_name_success() {
        let (container, handler) =
            setup_test_db_for_name_queries().await.unwrap();

        // Use a unique name to avoid cache conflicts
        let unique_name = format!(
            "Alice_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        );
        let user_id = create_test_user_with_name(&container, &unique_name)
            .await
            .unwrap();

        let query = GetUserByNameQuery {
            name: unique_name.clone(),
        };
        let result = handler.execute(query).await.unwrap();

        assert_eq!(result.id, user_id);
        assert_eq!(result.name, unique_name);
    }

    #[tokio::test]
    async fn test_get_user_by_name_not_found() {
        let (_container, handler) =
            setup_test_db_for_name_queries().await.unwrap();

        let query = GetUserByNameQuery {
            name: "NonExistentUser".to_string(),
        };
        let result = handler.execute(query).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            UserError::NameNotFound { username } => {
                assert_eq!(username, "NonExistentUser");
            }
            _ => panic!("Expected NameNotFound error"),
        }
    }

    async fn setup_test_db_for_list_queries()
    -> anyhow::Result<(TestPostgresContainer, ListUsersQueryHandler)> {
        let container = TestPostgresContainer::new().await?;
        let redis_container = TestRedisContainer::new().await?;
        redis_container.flush_db().await?;

        // Initialize the cache provider with the Redis pool
        CacheProvider::init_redis_static(redis_container.pool.clone());

        let sql_connect = create_sql_connect(&container);
        let handler = ListUsersQueryHandler::new(sql_connect);
        Ok((container, handler))
    }

    #[tokio::test]
    async fn test_list_users_with_limit() {
        let (container, handler) =
            setup_test_db_for_list_queries().await.unwrap();
        create_test_users(&container).await.unwrap();

        let query = ListUsersQuery {
            limit: Some(1),
            offset: None,
        };
        let result = handler.execute(query).await.unwrap();

        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn test_list_users_with_offset() {
        let (container, handler) =
            setup_test_db_for_list_queries().await.unwrap();
        create_test_users(&container).await.unwrap();

        let query = ListUsersQuery {
            limit: None,
            offset: Some(1),
        };
        let result = handler.execute(query).await.unwrap();
        assert!(!result.is_empty());
    }

    #[tokio::test]
    async fn test_list_users_empty() {
        let (_container, handler) =
            setup_test_db_for_list_queries().await.unwrap();

        let query = ListUsersQuery {
            limit: None,
            offset: None,
        };
        let result = handler.execute(query).await.unwrap();

        // Test expects empty result since no users are created
        assert_eq!(result.len(), 0);
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
    ) -> Result<UserResponse, UserError> {
        let backend = CacheProvider::get_backend();

        // Try to get from cache first
        let cache_key = UserByNameCacheKey;
        let mut cache = cache_key.bind_with(backend.clone(), &query.name);

        if let Ok(Some(user)) = cache.try_get().await {
            tracing::debug!("Cache hit for user by name {}", query.name);
            return Ok(UserResponse::from(user));
        }

        tracing::debug!(
            "Cache miss for user by name {}, fetching from DB",
            query.name
        );

        let user =
            self.user_dao.find_by_name(&query.name).await?.ok_or_else(
                || {
                    UserError::NameNotFound {
                        username: query.name,
                    }
                },
            )?;

        // Cache for 5 minutes - user data doesn't change often
        let _ = cache
            .set_with_expire::<()>(user.clone(), Duration::from_secs(300))
            .await;

        Ok(user.into())
    }
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
    ) -> Result<Vec<user_models::User>, UserError> {
        // For paginated queries, we'll cache with a specific key including
        // limit/offset For now, we'll only cache the default list (no
        // pagination)
        if query.limit.is_none() && query.offset.is_none() {
            let backend = CacheProvider::get_backend();

            let cache_key = UserListCacheKey;
            let mut cache = cache_key.bind(backend);

            if let Ok(Some(users)) = cache.try_get().await {
                tracing::debug!("Cache hit for user list");
                return Ok(users);
            }

            tracing::debug!("Cache miss for user list, fetching from DB");

            let users =
                self.user_dao.find_with_pagination(None, None).await?;

            // Cache for 2 minutes - list might change more often than
            // individual users
            let _ = cache
                .set_with_expire::<()>(
                    users.clone(),
                    Duration::from_secs(120),
                )
                .await;

            Ok(users)
        }
        else {
            let users = self
                .user_dao
                .find_with_pagination(query.limit, query.offset)
                .await?;
            Ok(users)
        }
    }
}
