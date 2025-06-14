#![allow(unused)]
use std::{borrow::Cow, marker::PhantomData, time::Duration};

use bytes::Bytes;
use deadpool_redis::redis::{AsyncCommands, FromRedisValue, RedisResult};
use moka::future::Cache;

use crate::core::{value::{Json, CacheValue}, type_bind::RedisTypeTrait};
use serde::{Serialize, Deserialize};

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
    T: Serialize + for<'de> Deserialize<'de> + Send + Sync + 'redis,
{
    pub async fn exists<RV>(&mut self) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.redis.exists(&*self.key).await
    }

    pub async fn set<RV>(&mut self, value: impl Into<Json<T>>) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.redis.set(&*self.key, value.into()).await
    }

    pub async fn set_if_not_exist<RV>(
        &mut self, value: impl Into<Json<T>>,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.redis.set_nx(&*self.key, value.into()).await
    }

    pub async fn set_with_expire<RV>(
        &mut self, value: impl Into<Json<T>>, duration: Duration,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.redis
            .set_ex(&*self.key, value.into(), duration.as_secs() as _)
            .await
    }

    pub async fn get(&mut self) -> RedisResult<T> {
        let json: Json<T> = self.redis.get(&*self.key).await?;
        Ok(json.inner())
    }

    pub async fn try_get(&mut self) -> RedisResult<Option<T>> {
        Ok(if self.exists().await? {
            Some(self.get().await?)
        }
        else {
            None
        })
    }

    pub async fn remove<RV>(&mut self) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.redis.del(&*self.key).await
    }
}
