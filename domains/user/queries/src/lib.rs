use redis_connection::cache_key;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: uuid::Uuid,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<user_models::User> for UserResponse {
    fn from(user: user_models::User) -> Self {
        Self {
            id: user.id,
            name: user.name,
            created_at: user.created_at,
        }
    }
}
#[derive(Debug, Deserialize, Clone)]
pub struct ListUsersQuery {
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}
#[derive(Debug, Deserialize)]
pub struct GetUserQuery {
    pub user_id: Uuid,
}
#[derive(Debug, Deserialize)]
pub struct GetUserByNameQuery {
    pub name: String,
}

cache_key!(UserCacheKey::<user_models::User> => "user:{}"[id: Uuid]);
cache_key!(UserByNameCacheKey::<user_models::User> => "user:name:{}"[name: String]);
cache_key!(UserListCacheKey::<Vec<user_models::User>> => "users:list");
