use redis_connection::cache_key;
use uuid::Uuid;

cache_key!(UserCacheKey::<user_models::User> => "user:{}"[id: Uuid]);
cache_key!(UserByNameCacheKey::<user_models::User> => "user:name:{}"[name: String]);
cache_key!(UserListCacheKey::<Vec<user_models::User>> => "users:list");
