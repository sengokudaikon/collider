#![allow(unused)]
use std::{borrow::Cow, collections::HashMap, marker::PhantomData};

use bytes::Bytes;
use deadpool_redis::redis::{
    AsyncCommands, FromRedisValue, RedisResult, ToRedisArgs,
};
use moka::future::Cache;
use serde::{Deserialize, Serialize};

use crate::core::{
    backend::CacheBackend,
    type_bind::CacheTypeTrait,
    value::{CacheValue, Json},
};

pub struct Hash<T> {
    pool: deadpool_redis::Pool,
    key: Cow<'static, str>,
    __phantom: PhantomData<T>,
}

impl<T> CacheTypeTrait<'_> for Hash<T> {
    fn from_cache_and_key(
        backend: CacheBackend<'_>, key: Cow<'static, str>,
    ) -> Self {
        let pool = match backend {
            CacheBackend::Redis(pool) => pool,
            _ => panic!("Hash type can only be created from Redis backend"),
        };

        Self {
            pool,
            key,
            __phantom: PhantomData,
        }
    }
}

impl<T> Hash<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Send + Sync,
{
    pub async fn exists<'arg, RV, F>(&mut self, field: F) -> RedisResult<RV>
    where
        F: ToRedisArgs + Send + Sync + 'arg,
        RV: FromRedisValue,
    {
        let mut conn = self.pool.get().await.map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::IoError,
                "Pool connection error",
                e.to_string(),
            ))
        })?;
        conn.hexists(&*self.key, field).await
    }

    pub async fn set<'arg, RV, F>(
        &mut self, field: F, value: impl Into<Json<T>>,
    ) -> RedisResult<RV>
    where
        F: ToRedisArgs + Send + Sync + 'arg,
        RV: FromRedisValue,
    {
        let mut conn = self.pool.get().await.map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::IoError,
                "Pool connection error",
                e.to_string(),
            ))
        })?;
        conn.hset(&*self.key, field, value.into()).await
    }

    pub async fn get<'arg, F>(&mut self, field: F) -> RedisResult<T>
    where
        F: ToRedisArgs + Send + Sync + 'arg,
    {
        let mut conn = self.pool.get().await.map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::IoError,
                "Pool connection error",
                e.to_string(),
            ))
        })?;
        let json: Json<T> = conn.hget(&*self.key, field).await?;
        Ok(json.inner())
    }

    pub async fn all<K>(&mut self) -> RedisResult<HashMap<K, T>>
    where
        K: FromRedisValue + Eq + std::hash::Hash,
    {
        let mut conn = self.pool.get().await.map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::IoError,
                "Pool connection error",
                e.to_string(),
            ))
        })?;
        let map: HashMap<K, Json<T>> = conn.hgetall(&*self.key).await?;
        Ok(map.into_iter().map(|(k, v)| (k, v.inner())).collect())
    }

    pub async fn try_get<'arg, F>(
        &mut self, field: F,
    ) -> RedisResult<Option<T>>
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

    pub async fn remove<'arg, RV, F>(&mut self, field: F) -> RedisResult<RV>
    where
        F: ToRedisArgs + Send + Sync + 'arg,
        RV: FromRedisValue,
    {
        let mut conn = self.pool.get().await.map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::IoError,
                "Pool connection error",
                e.to_string(),
            ))
        })?;
        conn.hdel(&*self.key, field).await
    }
}
