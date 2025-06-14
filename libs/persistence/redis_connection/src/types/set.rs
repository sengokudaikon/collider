#![allow(unused)]
use std::{borrow::Cow, collections::HashSet, marker::PhantomData};

use bytes::Bytes;
use deadpool_redis::redis::{
    AsyncCommands, FromRedisValue, RedisResult, ToRedisArgs,
};
use moka::future::Cache;
use serde::{Serialize, Deserialize};

use crate::core::{value::{Json, CacheValue}, type_bind::RedisTypeTrait};

pub struct Set<'redis, R: 'redis, T> {
    redis: &'redis mut R,
    key: Cow<'static, str>,
    __phantom: PhantomData<T>,
}

impl<'redis, R, T> RedisTypeTrait<'redis, R> for Set<'redis, R, T> {
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

impl<'redis, R, T> Set<'redis, R, T>
where
    R: redis::aio::ConnectionLike + Send + Sync,
    T: Serialize + for<'de> Deserialize<'de> + Send + Sync + 'redis + std::hash::Hash + Eq,
{
    /// Add one or more members to a set
    pub async fn add<RV>(&mut self, value: impl Into<Json<T>>) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.redis.sadd(&*self.key, value.into()).await
    }

    /// Add multiple members to a set
    pub async fn add_multiple<RV>(&mut self, values: Vec<impl Into<Json<T>>>) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        // Convert all values to Json
        let json_values: Vec<Json<T>> = values.into_iter().map(|v| v.into()).collect();
        // Redis can handle multiple values in a single sadd call
        self.redis.sadd(&*self.key, json_values).await
    }

    /// Remove one or more members from a set
    pub async fn remove<RV>(&mut self, value: impl Into<Json<T>>) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.redis.srem(&*self.key, value.into()).await
    }

    /// Check if a member exists in the set
    pub async fn contains<RV>(&mut self, value: impl Into<Json<T>>) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.redis.sismember(&*self.key, value.into()).await
    }

    /// Get all members of the set
    pub async fn members(&mut self) -> RedisResult<HashSet<T>> {
        let json_set: HashSet<Json<T>> = self.redis.smembers(&*self.key).await?;
        Ok(json_set.into_iter().map(|json| json.inner()).collect())
    }

    /// Get the number of members in the set
    pub async fn len<RV>(&mut self) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.redis.scard(&*self.key).await
    }

    /// Remove and return a random member from the set
    pub async fn pop(&mut self) -> RedisResult<Option<T>> {
        let result: Option<Json<T>> = self.redis.spop(&*self.key).await?;
        Ok(result.map(|json| json.inner()))
    }

    /// Return random members from the set without removing them
    pub async fn random_members(&mut self, count: usize) -> RedisResult<Vec<T>> {
        let json_vec: Vec<Json<T>> = self.redis.srandmember_multiple(&*self.key, count).await?;
        Ok(json_vec.into_iter().map(|json| json.inner()).collect())
    }

    /// Compute the union of multiple sets
    pub async fn union(&mut self, other_keys: &[&str]) -> RedisResult<HashSet<T>> {
        let mut keys = vec![&*self.key];
        keys.extend(other_keys);
        let json_set: HashSet<Json<T>> = self.redis.sunion(&keys).await?;
        Ok(json_set.into_iter().map(|json| json.inner()).collect())
    }

    /// Compute the intersection of multiple sets
    pub async fn intersect(&mut self, other_keys: &[&str]) -> RedisResult<HashSet<T>> {
        let mut keys = vec![&*self.key];
        keys.extend(other_keys);
        let json_set: HashSet<Json<T>> = self.redis.sinter(&keys).await?;
        Ok(json_set.into_iter().map(|json| json.inner()).collect())
    }

    /// Compute the difference between sets
    pub async fn diff(&mut self, other_keys: &[&str]) -> RedisResult<HashSet<T>> {
        let mut keys = vec![&*self.key];
        keys.extend(other_keys);
        let json_set: HashSet<Json<T>> = self.redis.sdiff(&keys).await?;
        Ok(json_set.into_iter().map(|json| json.inner()).collect())
    }

    /// Move member from one set to another
    pub async fn move_to<RV>(&mut self, dest_key: &str, value: impl Into<Json<T>>) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.redis.smove(&*self.key, dest_key, value.into()).await
    }
}