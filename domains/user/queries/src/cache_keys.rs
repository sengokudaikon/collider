use redis_connection::{core::value::Json, redis_key};
use user_models::User;
use uuid::Uuid;

// Tiered cache keys for user domain

// User entity caching
redis_key!(UserCacheKey::<Json<User>> => "user:{}"[id: Uuid]);
redis_key!(UserByNameCacheKey::<Json<User>> => "user:name:{}"[name: String]);
redis_key!(UserListCacheKey::<Json<Vec<User>>> => "users:list");
