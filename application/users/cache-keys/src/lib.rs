use redis_connection::cache_key;

cache_key!(UserCacheKey::<user_models::User> => "user:{}"[id: i64]);
cache_key!(UserByNameCacheKey::<user_models::User> => "user:name:{}"[name: String]);
cache_key!(UserListCacheKey::<Vec<user_models::User>> => "users:list");
