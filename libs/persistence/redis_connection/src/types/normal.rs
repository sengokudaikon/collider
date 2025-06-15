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

pub struct Normal<T> {
    pool: deadpool_redis::Pool,
    key: Cow<'static, str>,
    __phantom: PhantomData<T>,
}

impl<T> CacheTypeTrait<'_> for Normal<T> {
    fn from_cache_and_key(
        backend: super::super::core::backend::CacheBackend<'_>,
        key: Cow<'static, str>,
    ) -> Self {
        let pool = match backend {
            super::super::core::backend::CacheBackend::Redis(pool) => pool,
            _ => panic!("Normal type can only be created from Redis backend"),
        };

        Self {
            pool,
            key,
            __phantom: PhantomData,
        }
    }
}

impl<T> Normal<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Send + Sync,
{
    pub async fn exists<RV>(&mut self) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        let mut conn = self.pool.get().await.map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::IoError,
                "Pool connection error",
                e.to_string(),
            ))
        })?;
        conn.exists(&*self.key).await
    }

    pub async fn set<RV>(
        &mut self, value: impl Into<Json<T>>,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        let mut conn = self.pool.get().await.map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::IoError,
                "Pool connection error",
                e.to_string(),
            ))
        })?;
        conn.set(&*self.key, value.into()).await
    }

    pub async fn set_if_not_exist<RV>(
        &mut self, value: impl Into<Json<T>>,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        let mut conn = self.pool.get().await.map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::IoError,
                "Pool connection error",
                e.to_string(),
            ))
        })?;
        conn.set_nx(&*self.key, value.into()).await
    }

    pub async fn set_with_expire<RV>(
        &mut self, value: impl Into<Json<T>>, duration: Duration,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        let mut conn = self.pool.get().await.map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::IoError,
                "Pool connection error",
                e.to_string(),
            ))
        })?;
        conn.set_ex(&*self.key, value.into(), duration.as_secs() as _)
            .await
    }

    pub async fn get(&mut self) -> RedisResult<T> {
        let mut conn = self.pool.get().await.map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::IoError,
                "Pool connection error",
                e.to_string(),
            ))
        })?;
        let json: Json<T> = conn.get(&*self.key).await?;
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
        let mut conn = self.pool.get().await.map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::IoError,
                "Pool connection error",
                e.to_string(),
            ))
        })?;
        conn.del(&*self.key).await
    }
}
