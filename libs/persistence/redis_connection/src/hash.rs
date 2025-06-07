#![allow(unused)]
use std::{borrow::Cow, collections::HashMap, marker::PhantomData};

use bytes::Bytes;
use deadpool_redis::redis::{
    AsyncCommands, FromRedisValue, RedisResult, ToRedisArgs,
};
use moka::future::Cache;

use super::{redis_value::RedisValue, type_bind::RedisTypeTrait};

pub struct Hash<'redis, R: 'redis, T> {
    redis: &'redis mut R,
    key: Cow<'static, str>,
    __phantom: PhantomData<T>,
}

impl<'redis, R, T> RedisTypeTrait<'redis, R> for Hash<'redis, R, T> {
    fn from_redis_and_key(
        redis: &'redis mut R, key: Cow<'static, str>,
        memory: Option<Cache<String, Bytes>>,
    ) -> Self {
        Self {
            redis,
            key,
            __phantom: PhantomData,
        }
    }
}

impl<'redis, R, T> Hash<'redis, R, T>
where
    R: redis::aio::ConnectionLike + Send + Sync,
    T: RedisValue<'redis>,
{
    pub async fn exists<'arg, RV, F>(&mut self, field: F) -> RedisResult<RV>
    where
        F: ToRedisArgs + Send + Sync + 'arg,
        RV: FromRedisValue,
    {
        self.redis.hexists(&*self.key, field).await
    }

    pub async fn set<'arg, RV, F>(
        &mut self, field: F, value: T::Input,
    ) -> RedisResult<RV>
    where
        F: ToRedisArgs + Send + Sync + 'arg,
        RV: FromRedisValue,
    {
        self.redis.hset(&*self.key, field, value).await
    }

    /// Get the corresponding value of the corresponding field in the current
    /// hash
    ///
    /// ## reference
    /// - [`AsyncCommands::hget`]
    pub async fn get<'arg, F>(&mut self, field: F) -> RedisResult<T::Output>
    where
        F: ToRedisArgs + Send + Sync + 'arg,
    {
        self.redis.hget(&*self.key, field).await
    }

    /// Get the corresponding value of the corresponding field in the current
    /// hash
    ///
    /// ## reference
    /// - [`AsyncCommands::hall`]
    pub async fn all<K>(&mut self) -> RedisResult<HashMap<K, T::Output>>
    where
        K: FromRedisValue + Eq + std::hash::Hash,
    {
        self.redis.hgetall(&*self.key).await
    }

    /// Try to get the corresponding value of the corresponding field in the
    /// current hash. If it does not exist, [`None`] will be returned.
    ///
    /// ## reference
    /// - [`AsyncCommands::hexists`]
    /// - [`AsyncCommands::hget`]
    /// - [`Hash::get`]
    /// - [`Hash::exists`]
    pub async fn try_get<'arg, F>(
        &mut self, field: F,
    ) -> RedisResult<Option<T::Output>>
    where
        F: ToRedisArgs + Send + Sync + 'arg + Copy,
    {
        Ok(if self.exists(field).await? {
            Some(self.get(field).await?)
        }
        else {
            None
        })
    }

    /// Try to delete the corresponding value of the corresponding field in
    /// the current hash
    ///
    /// ## reference
    /// - [`AsyncCommands::hdel`]
    pub async fn remove<'arg, RV, F>(&mut self, field: F) -> RedisResult<RV>
    where
        F: ToRedisArgs + Send + Sync + 'arg,
        RV: FromRedisValue,
    {
        self.redis.hdel(&*self.key, field).await
    }
}
