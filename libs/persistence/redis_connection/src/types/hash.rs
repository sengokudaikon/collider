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

pub struct Hash<'cache, T> {
    redis: &'cache mut deadpool_redis::Connection,
    key: Cow<'static, str>,
    __phantom: PhantomData<T>,
}

impl<'cache, T> CacheTypeTrait<'cache> for Hash<'cache, T> {
    fn from_cache_and_key(
        backend: CacheBackend<'cache>, key: Cow<'static, str>,
    ) -> Self {
        let redis = match backend {
            CacheBackend::Redis(redis) => redis,
            _ => panic!("Hash type can only be created from Redis backend"),
        };

        Self {
            redis,
            key,
            __phantom: PhantomData,
        }
    }
}

impl<'cache, T> Hash<'cache, T>
where
    T: Serialize + for<'de> Deserialize<'de> + Send + Sync + 'cache,
{
    pub async fn exists<'arg, RV, F>(&mut self, field: F) -> RedisResult<RV>
    where
        F: ToRedisArgs + Send + Sync + 'arg,
        RV: FromRedisValue,
    {
        self.redis.hexists(&*self.key, field).await
    }

    pub async fn set<'arg, RV, F>(
        &mut self, field: F, value: impl Into<Json<T>>,
    ) -> RedisResult<RV>
    where
        F: ToRedisArgs + Send + Sync + 'arg,
        RV: FromRedisValue,
    {
        self.redis.hset(&*self.key, field, value.into()).await
    }

    pub async fn get<'arg, F>(&mut self, field: F) -> RedisResult<T>
    where
        F: ToRedisArgs + Send + Sync + 'arg,
    {
        let json: Json<T> = self.redis.hget(&*self.key, field).await?;
        Ok(json.inner())
    }

    pub async fn all<K>(&mut self) -> RedisResult<HashMap<K, T>>
    where
        K: FromRedisValue + Eq + std::hash::Hash,
    {
        let map: HashMap<K, Json<T>> = self.redis.hgetall(&*self.key).await?;
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
        self.redis.hdel(&*self.key, field).await
    }
}
