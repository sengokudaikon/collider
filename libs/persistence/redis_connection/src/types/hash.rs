#![allow(unused)]
use std::{borrow::Cow, collections::HashMap, marker::PhantomData};

use bytes::Bytes;
use deadpool_redis::redis::{
    AsyncCommands, FromRedisValue, RedisResult, ToRedisArgs,
};
use moka::future::Cache;

use crate::core::{value::{Json, CacheValue}, type_bind::RedisTypeTrait};
use serde::{Serialize, Deserialize};

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
    T: Serialize + for<'de> Deserialize<'de> + Send + Sync + 'redis,
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
