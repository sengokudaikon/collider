use redis_connection::{cache_key, core::value::Json};
use user_models::User;
use uuid::Uuid;

// Tiered cache keys for user domain

// User entity caching
cache_key!(UserCacheKey::<Json<User>> => "user:{}"[id: Uuid]);
cache_key!(UserByNameCacheKey::<Json<User>> => "user:name:{}"[name: String]);
cache_key!(UserListCacheKey::<Json<Vec<User>>> => "users:list");
