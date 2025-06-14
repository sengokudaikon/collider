use std::collections::{HashMap, HashSet};
use std::time::Duration;

use async_trait::async_trait;
use deadpool_redis::redis::{FromRedisValue, RedisResult, ToRedisArgs};

/// A high-level trait for Redis commands that abstracts away the underlying
/// redis::AsyncCommands trait and provides a cleaner, more ergonomic interface.
#[async_trait]
pub trait RedisCommands {
    // String commands
    async fn get<K, V>(&mut self, key: K) -> RedisResult<V>
    where
        K: ToRedisArgs + Send + Sync,
        V: FromRedisValue;

    async fn set<K, V>(&mut self, key: K, value: V) -> RedisResult<()>
    where
        K: ToRedisArgs + Send + Sync,
        V: ToRedisArgs + Send + Sync;

    async fn set_ex<K, V>(&mut self, key: K, value: V, seconds: u64) -> RedisResult<()>
    where
        K: ToRedisArgs + Send + Sync,
        V: ToRedisArgs + Send + Sync;

    async fn set_nx<K, V>(&mut self, key: K, value: V) -> RedisResult<bool>
    where
        K: ToRedisArgs + Send + Sync,
        V: ToRedisArgs + Send + Sync;

    async fn get_set<K, V, R>(&mut self, key: K, value: V) -> RedisResult<R>
    where
        K: ToRedisArgs + Send + Sync,
        V: ToRedisArgs + Send + Sync,
        R: FromRedisValue;

    // Key commands
    async fn exists<K>(&mut self, key: K) -> RedisResult<bool>
    where
        K: ToRedisArgs + Send + Sync;

    async fn del<K>(&mut self, key: K) -> RedisResult<i32>
    where
        K: ToRedisArgs + Send + Sync;

    async fn expire<K>(&mut self, key: K, seconds: u64) -> RedisResult<bool>
    where
        K: ToRedisArgs + Send + Sync;

    async fn ttl<K>(&mut self, key: K) -> RedisResult<i64>
    where
        K: ToRedisArgs + Send + Sync;

    async fn rename<K1, K2>(&mut self, old_key: K1, new_key: K2) -> RedisResult<()>
    where
        K1: ToRedisArgs + Send + Sync,
        K2: ToRedisArgs + Send + Sync;

    // Hash commands
    async fn hget<K, F, V>(&mut self, key: K, field: F) -> RedisResult<V>
    where
        K: ToRedisArgs + Send + Sync,
        F: ToRedisArgs + Send + Sync,
        V: FromRedisValue;

    async fn hset<K, F, V>(&mut self, key: K, field: F, value: V) -> RedisResult<bool>
    where
        K: ToRedisArgs + Send + Sync,
        F: ToRedisArgs + Send + Sync,
        V: ToRedisArgs + Send + Sync;

    async fn hdel<K, F>(&mut self, key: K, field: F) -> RedisResult<i32>
    where
        K: ToRedisArgs + Send + Sync,
        F: ToRedisArgs + Send + Sync;

    async fn hexists<K, F>(&mut self, key: K, field: F) -> RedisResult<bool>
    where
        K: ToRedisArgs + Send + Sync,
        F: ToRedisArgs + Send + Sync;

    async fn hgetall<K, FK, FV>(&mut self, key: K) -> RedisResult<HashMap<FK, FV>>
    where
        K: ToRedisArgs + Send + Sync,
        FK: FromRedisValue + Eq + std::hash::Hash,
        FV: FromRedisValue;

    async fn hkeys<K, F>(&mut self, key: K) -> RedisResult<Vec<F>>
    where
        K: ToRedisArgs + Send + Sync,
        F: FromRedisValue;

    async fn hvals<K, V>(&mut self, key: K) -> RedisResult<Vec<V>>
    where
        K: ToRedisArgs + Send + Sync,
        V: FromRedisValue;

    // List commands
    async fn lpush<K, V>(&mut self, key: K, value: V) -> RedisResult<i32>
    where
        K: ToRedisArgs + Send + Sync,
        V: ToRedisArgs + Send + Sync;

    async fn rpush<K, V>(&mut self, key: K, value: V) -> RedisResult<i32>
    where
        K: ToRedisArgs + Send + Sync,
        V: ToRedisArgs + Send + Sync;

    async fn lpop<K, V>(&mut self, key: K) -> RedisResult<Option<V>>
    where
        K: ToRedisArgs + Send + Sync,
        V: FromRedisValue;

    async fn rpop<K, V>(&mut self, key: K) -> RedisResult<Option<V>>
    where
        K: ToRedisArgs + Send + Sync,
        V: FromRedisValue;

    async fn llen<K>(&mut self, key: K) -> RedisResult<i32>
    where
        K: ToRedisArgs + Send + Sync;

    async fn lrange<K, V>(&mut self, key: K, start: i32, stop: i32) -> RedisResult<Vec<V>>
    where
        K: ToRedisArgs + Send + Sync,
        V: FromRedisValue;

    // Set commands
    async fn sadd<K, V>(&mut self, key: K, member: V) -> RedisResult<i32>
    where
        K: ToRedisArgs + Send + Sync,
        V: ToRedisArgs + Send + Sync;

    async fn srem<K, V>(&mut self, key: K, member: V) -> RedisResult<i32>
    where
        K: ToRedisArgs + Send + Sync,
        V: ToRedisArgs + Send + Sync;

    async fn sismember<K, V>(&mut self, key: K, member: V) -> RedisResult<bool>
    where
        K: ToRedisArgs + Send + Sync,
        V: ToRedisArgs + Send + Sync;

    async fn smembers<K, V>(&mut self, key: K) -> RedisResult<HashSet<V>>
    where
        K: ToRedisArgs + Send + Sync,
        V: FromRedisValue + Eq + std::hash::Hash;

    async fn scard<K>(&mut self, key: K) -> RedisResult<i32>
    where
        K: ToRedisArgs + Send + Sync;

    // Utility commands
    async fn ping(&mut self) -> RedisResult<String>;
    
    async fn flushdb(&mut self) -> RedisResult<()>;
}

/// A Redis command executor that wraps a connection and implements RedisCommands
pub struct RedisCommandExecutor<C> {
    connection: C,
}

impl<C> RedisCommandExecutor<C> {
    pub fn new(connection: C) -> Self {
        Self { connection }
    }

    pub fn into_inner(self) -> C {
        self.connection
    }

    pub fn as_connection(&self) -> &C {
        &self.connection
    }

    pub fn as_connection_mut(&mut self) -> &mut C {
        &mut self.connection
    }
}

#[async_trait]
impl<C> RedisCommands for RedisCommandExecutor<C>
where
    C: deadpool_redis::redis::aio::ConnectionLike + Send + Sync,
{
    // String commands
    async fn get<K, V>(&mut self, key: K) -> RedisResult<V>
    where
        K: ToRedisArgs + Send + Sync,
        V: FromRedisValue,
    {
        use deadpool_redis::redis::AsyncCommands;
        self.connection.get(key).await
    }

    async fn set<K, V>(&mut self, key: K, value: V) -> RedisResult<()>
    where
        K: ToRedisArgs + Send + Sync,
        V: ToRedisArgs + Send + Sync,
    {
        use deadpool_redis::redis::AsyncCommands;
        self.connection.set(key, value).await
    }

    async fn set_ex<K, V>(&mut self, key: K, value: V, seconds: u64) -> RedisResult<()>
    where
        K: ToRedisArgs + Send + Sync,
        V: ToRedisArgs + Send + Sync,
    {
        use deadpool_redis::redis::AsyncCommands;
        self.connection.set_ex(key, value, seconds).await
    }

    async fn set_nx<K, V>(&mut self, key: K, value: V) -> RedisResult<bool>
    where
        K: ToRedisArgs + Send + Sync,
        V: ToRedisArgs + Send + Sync,
    {
        use deadpool_redis::redis::AsyncCommands;
        self.connection.set_nx(key, value).await
    }

    async fn get_set<K, V, R>(&mut self, key: K, value: V) -> RedisResult<R>
    where
        K: ToRedisArgs + Send + Sync,
        V: ToRedisArgs + Send + Sync,
        R: FromRedisValue,
    {
        use deadpool_redis::redis::AsyncCommands;
        self.connection.getset(key, value).await
    }

    // Key commands
    async fn exists<K>(&mut self, key: K) -> RedisResult<bool>
    where
        K: ToRedisArgs + Send + Sync,
    {
        use deadpool_redis::redis::AsyncCommands;
        self.connection.exists(key).await
    }

    async fn del<K>(&mut self, key: K) -> RedisResult<i32>
    where
        K: ToRedisArgs + Send + Sync,
    {
        use deadpool_redis::redis::AsyncCommands;
        self.connection.del(key).await
    }

    async fn expire<K>(&mut self, key: K, seconds: u64) -> RedisResult<bool>
    where
        K: ToRedisArgs + Send + Sync,
    {
        use deadpool_redis::redis::AsyncCommands;
        self.connection.expire(key, seconds as i64).await
    }

    async fn ttl<K>(&mut self, key: K) -> RedisResult<i64>
    where
        K: ToRedisArgs + Send + Sync,
    {
        use deadpool_redis::redis::AsyncCommands;
        self.connection.ttl(key).await
    }

    async fn rename<K1, K2>(&mut self, old_key: K1, new_key: K2) -> RedisResult<()>
    where
        K1: ToRedisArgs + Send + Sync,
        K2: ToRedisArgs + Send + Sync,
    {
        use deadpool_redis::redis::AsyncCommands;
        self.connection.rename(old_key, new_key).await
    }

    // Hash commands
    async fn hget<K, F, V>(&mut self, key: K, field: F) -> RedisResult<V>
    where
        K: ToRedisArgs + Send + Sync,
        F: ToRedisArgs + Send + Sync,
        V: FromRedisValue,
    {
        use deadpool_redis::redis::AsyncCommands;
        self.connection.hget(key, field).await
    }

    async fn hset<K, F, V>(&mut self, key: K, field: F, value: V) -> RedisResult<bool>
    where
        K: ToRedisArgs + Send + Sync,
        F: ToRedisArgs + Send + Sync,
        V: ToRedisArgs + Send + Sync,
    {
        use deadpool_redis::redis::AsyncCommands;
        self.connection.hset(key, field, value).await
    }

    async fn hdel<K, F>(&mut self, key: K, field: F) -> RedisResult<i32>
    where
        K: ToRedisArgs + Send + Sync,
        F: ToRedisArgs + Send + Sync,
    {
        use deadpool_redis::redis::AsyncCommands;
        self.connection.hdel(key, field).await
    }

    async fn hexists<K, F>(&mut self, key: K, field: F) -> RedisResult<bool>
    where
        K: ToRedisArgs + Send + Sync,
        F: ToRedisArgs + Send + Sync,
    {
        use deadpool_redis::redis::AsyncCommands;
        self.connection.hexists(key, field).await
    }

    async fn hgetall<K, FK, FV>(&mut self, key: K) -> RedisResult<HashMap<FK, FV>>
    where
        K: ToRedisArgs + Send + Sync,
        FK: FromRedisValue + Eq + std::hash::Hash,
        FV: FromRedisValue,
    {
        use deadpool_redis::redis::AsyncCommands;
        self.connection.hgetall(key).await
    }

    async fn hkeys<K, F>(&mut self, key: K) -> RedisResult<Vec<F>>
    where
        K: ToRedisArgs + Send + Sync,
        F: FromRedisValue,
    {
        use deadpool_redis::redis::AsyncCommands;
        self.connection.hkeys(key).await
    }

    async fn hvals<K, V>(&mut self, key: K) -> RedisResult<Vec<V>>
    where
        K: ToRedisArgs + Send + Sync,
        V: FromRedisValue,
    {
        use deadpool_redis::redis::AsyncCommands;
        self.connection.hvals(key).await
    }

    // List commands
    async fn lpush<K, V>(&mut self, key: K, value: V) -> RedisResult<i32>
    where
        K: ToRedisArgs + Send + Sync,
        V: ToRedisArgs + Send + Sync,
    {
        use deadpool_redis::redis::AsyncCommands;
        self.connection.lpush(key, value).await
    }

    async fn rpush<K, V>(&mut self, key: K, value: V) -> RedisResult<i32>
    where
        K: ToRedisArgs + Send + Sync,
        V: ToRedisArgs + Send + Sync,
    {
        use deadpool_redis::redis::AsyncCommands;
        self.connection.rpush(key, value).await
    }

    async fn lpop<K, V>(&mut self, key: K) -> RedisResult<Option<V>>
    where
        K: ToRedisArgs + Send + Sync,
        V: FromRedisValue,
    {
        use deadpool_redis::redis::AsyncCommands;
        self.connection.lpop(key, None).await
    }

    async fn rpop<K, V>(&mut self, key: K) -> RedisResult<Option<V>>
    where
        K: ToRedisArgs + Send + Sync,
        V: FromRedisValue,
    {
        use deadpool_redis::redis::AsyncCommands;
        self.connection.rpop(key, None).await
    }

    async fn llen<K>(&mut self, key: K) -> RedisResult<i32>
    where
        K: ToRedisArgs + Send + Sync,
    {
        use deadpool_redis::redis::AsyncCommands;
        self.connection.llen(key).await
    }

    async fn lrange<K, V>(&mut self, key: K, start: i32, stop: i32) -> RedisResult<Vec<V>>
    where
        K: ToRedisArgs + Send + Sync,
        V: FromRedisValue,
    {
        use deadpool_redis::redis::AsyncCommands;
        self.connection.lrange(key, start as isize, stop as isize).await
    }

    // Set commands
    async fn sadd<K, V>(&mut self, key: K, member: V) -> RedisResult<i32>
    where
        K: ToRedisArgs + Send + Sync,
        V: ToRedisArgs + Send + Sync,
    {
        use deadpool_redis::redis::AsyncCommands;
        self.connection.sadd(key, member).await
    }

    async fn srem<K, V>(&mut self, key: K, member: V) -> RedisResult<i32>
    where
        K: ToRedisArgs + Send + Sync,
        V: ToRedisArgs + Send + Sync,
    {
        use deadpool_redis::redis::AsyncCommands;
        self.connection.srem(key, member).await
    }

    async fn sismember<K, V>(&mut self, key: K, member: V) -> RedisResult<bool>
    where
        K: ToRedisArgs + Send + Sync,
        V: ToRedisArgs + Send + Sync,
    {
        use deadpool_redis::redis::AsyncCommands;
        self.connection.sismember(key, member).await
    }

    async fn smembers<K, V>(&mut self, key: K) -> RedisResult<HashSet<V>>
    where
        K: ToRedisArgs + Send + Sync,
        V: FromRedisValue + Eq + std::hash::Hash,
    {
        use deadpool_redis::redis::AsyncCommands;
        self.connection.smembers(key).await
    }

    async fn scard<K>(&mut self, key: K) -> RedisResult<i32>
    where
        K: ToRedisArgs + Send + Sync,
    {
        use deadpool_redis::redis::AsyncCommands;
        self.connection.scard(key).await
    }

    // Utility commands
    async fn ping(&mut self) -> RedisResult<String> {
        use deadpool_redis::redis::AsyncCommands;
        self.connection.ping().await
    }

    async fn flushdb(&mut self) -> RedisResult<()> {
        use deadpool_redis::redis::AsyncCommands;
        self.connection.flushdb().await
    }
}

/// Extension trait to easily convert connections into command executors
pub trait IntoRedisCommands: Sized {
    fn cmd(self) -> RedisCommandExecutor<Self>;
}

impl<C> IntoRedisCommands for C
where
    C: deadpool_redis::redis::aio::ConnectionLike + Send + Sync,
{
    fn cmd(self) -> RedisCommandExecutor<Self> {
        RedisCommandExecutor::new(self)
    }
}

/// Builder for SET commands with options
pub struct SetCommandBuilder<K, V> {
    key: K,
    value: V,
    expire: Option<Duration>,
    only_if_not_exists: bool,
    only_if_exists: bool,
}

impl<K, V> SetCommandBuilder<K, V> {
    pub fn new(key: K, value: V) -> Self {
        Self {
            key,
            value,
            expire: None,
            only_if_not_exists: false,
            only_if_exists: false,
        }
    }

    pub fn expire_in(mut self, duration: Duration) -> Self {
        self.expire = Some(duration);
        self
    }

    pub fn only_if_not_exists(mut self) -> Self {
        self.only_if_not_exists = true;
        self
    }

    pub fn only_if_exists(mut self) -> Self {
        self.only_if_exists = true;
        self
    }

    pub async fn execute<C>(self, commands: &mut C) -> RedisResult<bool>
    where
        C: RedisCommands,
        K: ToRedisArgs + Send + Sync,
        V: ToRedisArgs + Send + Sync,
    {
        match (self.expire, self.only_if_not_exists, self.only_if_exists) {
            (Some(duration), true, false) => {
                // SET key value EX seconds NX
                let exists = commands.exists(&self.key).await?;
                if exists {
                    return Ok(false);
                }
                commands.set_ex(self.key, self.value, duration.as_secs()).await?;
                Ok(true)
            }
            (None, true, false) => {
                // SET key value NX
                commands.set_nx(self.key, self.value).await
            }
            (Some(duration), false, false) => {
                // SET key value EX seconds
                commands.set_ex(self.key, self.value, duration.as_secs()).await?;
                Ok(true)
            }
            (None, false, false) => {
                // SET key value
                commands.set(self.key, self.value).await?;
                Ok(true)
            }
            _ => {
                // For other combinations, fall back to basic SET
                commands.set(self.key, self.value).await?;
                Ok(true)
            }
        }
    }
}

/// Extension trait for more ergonomic command building
pub trait RedisCommandsExt: RedisCommands {
    fn set_builder<K, V>(&mut self, key: K, value: V) -> SetCommandBuilder<K, V> {
        SetCommandBuilder::new(key, value)
    }
}

impl<T: RedisCommands> RedisCommandsExt for T {}