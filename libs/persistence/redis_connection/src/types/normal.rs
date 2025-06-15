#![allow(unused)]
use std::{borrow::Cow, marker::PhantomData, time::Duration};

use bytes::Bytes;
use deadpool_redis::redis::{AsyncCommands, FromRedisValue, RedisResult};
use moka::future::Cache;
use serde::{Deserialize, Serialize};

use crate::core::{
    type_bind::CacheTypeTrait,
    value::{CacheValue, Json},
};

pub struct Normal<'cache, T> {
    redis: &'cache mut deadpool_redis::Connection,
    key: Cow<'static, str>,
    __phantom: PhantomData<T>,
}

impl<'cache, T> CacheTypeTrait<'cache> for Normal<'cache, T> {
    fn from_cache_and_key(
        backend: super::super::core::backend::CacheBackend<'cache>,
        key: Cow<'static, str>,
    ) -> Self {
        let redis = match backend {
            super::super::core::backend::CacheBackend::Redis(redis) => redis,
            _ => panic!("Normal type can only be created from Redis backend"),
        };

        Self {
            redis,
            key,
            __phantom: PhantomData,
        }
    }
}

impl<'cache, T> Normal<'cache, T>
where
    T: Serialize + for<'de> Deserialize<'de> + Send + Sync + 'cache,
{
    pub async fn exists<RV>(&mut self) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.redis.exists(&*self.key).await
    }

    pub async fn set<RV>(
        &mut self, value: impl Into<Json<T>>,
    ) -> RedisResult<RV>
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
