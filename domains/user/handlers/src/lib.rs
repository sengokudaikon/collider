pub mod commands;
pub mod queries;

pub use commands::{CreateUserHandler, DeleteUserHandler, UpdateUserHandler};
pub use queries::{
    GetUserByNameQueryHandler, GetUserQueryHandler, ListUsersQueryHandler,
};
use redis_connection::cache_key;
use uuid::Uuid;

cache_key!(UserCacheKey::<user_models::User> => "user:{}"[id: Uuid]);
cache_key!(UserByNameCacheKey::<user_models::User> => "user:name:{}"[name: String]);
cache_key!(UserListCacheKey::<Vec<user_models::User>> => "users:list");
