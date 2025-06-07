#![allow(unused)]
use std::{borrow::Cow, marker::PhantomData, time::Duration};

use bytes::Bytes;
use deadpool_redis::redis::{AsyncCommands, FromRedisValue, RedisResult};
use moka::future::Cache;

use super::{redis_value::RedisValue, type_bind::RedisTypeTrait};

pub struct Normal<'redis, R, T> {
    redis: &'redis mut R,
    key: Cow<'static, str>,
    __phantom: PhantomData<T>,
}

impl<'redis, R, T> RedisTypeTrait<'redis, R> for Normal<'redis, R, T> {
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

impl<'redis, R, T> Normal<'redis, R, T>
where
    R: redis::aio::ConnectionLike + Send + Sync,
    T: RedisValue<'redis>,
{
    /// Determine whether the current value exists
    ///
    /// ## reference
    /// - [`AsyncCommands::exists`]
    pub async fn exists<RV>(&mut self) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.redis.exists(&*self.key).await
    }

    /// write current value
    ///
    /// ## reference
    /// - [`AsyncCommands::set`]
    pub async fn set<RV>(&mut self, value: T::Input) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.redis.set(&*self.key, value).await
    }

    /// When the value does not exist, write the value    
    ///
    /// ## reference
    /// - [`AsyncCommands::set_nx`]
    pub async fn set_if_not_exist<RV>(
        &mut self, value: T::Input,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.redis.set_nx(&*self.key, value).await
    }

    /// Write the value and add expiration
    ///
    /// ## reference
    /// - [`AsyncCommands::set_ex`]
    pub async fn set_with_expire<RV>(
        &mut self, value: T::Input, duration: Duration,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.redis
            .set_ex(&*self.key, value, duration.as_secs() as _)
            .await
    }

    /// Get value
    ///
    /// ## reference
    /// - [`AsyncCommands::get`]
    pub async fn get(&mut self) -> RedisResult<T::Output> {
        self.redis.get(&*self.key).await
    }

    /// Try to get the value, if it does not exist, return [`None`]
    ///
    /// ## reference
    /// - [`AsyncCommands::get`]
    /// - [`AsyncCommands::exists`]
    pub async fn try_get(&mut self) -> RedisResult<Option<T::Output>> {
        Ok(if self.exists().await? {
            Some(self.get().await?)
        }
        else {
            None
        })
    }

    /// Delete value
    ///
    /// ## reference
    /// - [`AsyncCommands::del`]
    pub async fn remove<RV>(&mut self) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.redis.del(&*self.key).await
    }
}
