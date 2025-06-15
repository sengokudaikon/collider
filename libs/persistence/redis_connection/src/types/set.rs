#![allow(unused)]
use std::{borrow::Cow, collections::HashSet, marker::PhantomData};

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

pub struct Set<T> {
    pool: deadpool_redis::Pool,
    key: Cow<'static, str>,
    __phantom: PhantomData<T>,
}

impl<T> CacheTypeTrait<'_> for Set<T> {
    fn from_cache_and_key(
        backend: CacheBackend<'_>, key: Cow<'static, str>,
    ) -> Self {
        let pool = match backend {
            CacheBackend::Redis(pool) => pool,
            _ => panic!("Set type can only be created from Redis backend"),
        };

        Self {
            pool,
            key,
            __phantom: PhantomData,
        }
    }
}

impl<T> Set<T>
where
    T: Serialize
        + for<'de> Deserialize<'de>
        + Send
        + Sync
        + std::hash::Hash
        + Eq,
{
    /// Add one or more members to a set
    pub async fn add<RV>(
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
        conn.sadd(&*self.key, value.into()).await
    }

    /// Add multiple members to a set
    pub async fn add_multiple<RV>(
        &mut self, values: Vec<impl Into<Json<T>>>,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        // Convert all values to Json
        let json_values: Vec<Json<T>> =
            values.into_iter().map(|v| v.into()).collect();
        // Redis can handle multiple values in a single sadd call
        let mut conn = self.pool.get().await.map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::IoError,
                "Pool connection error",
                e.to_string(),
            ))
        })?;
        conn.sadd(&*self.key, json_values).await
    }

    /// Remove one or more members from a set
    pub async fn remove<RV>(
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
        conn.srem(&*self.key, value.into()).await
    }

    /// Check if a member exists in the set
    pub async fn contains<RV>(
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
        conn.sismember(&*self.key, value.into()).await
    }

    /// Get all members of the set
    pub async fn members(&mut self) -> RedisResult<HashSet<T>> {
        let mut conn = self.pool.get().await.map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::IoError,
                "Pool connection error",
                e.to_string(),
            ))
        })?;
        let json_set: HashSet<Json<T>> = conn.smembers(&*self.key).await?;
        Ok(json_set.into_iter().map(|json| json.inner()).collect())
    }

    /// Get the number of members in the set
    pub async fn len<RV>(&mut self) -> RedisResult<RV>
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
        conn.scard(&*self.key).await
    }

    /// Remove and return a random member from the set
    pub async fn pop(&mut self) -> RedisResult<Option<T>> {
        let mut conn = self.pool.get().await.map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::IoError,
                "Pool connection error",
                e.to_string(),
            ))
        })?;
        let result: Option<Json<T>> = conn.spop(&*self.key).await?;
        Ok(result.map(|json| json.inner()))
    }

    /// Return random members from the set without removing them
    pub async fn random_members(
        &mut self, count: usize,
    ) -> RedisResult<Vec<T>> {
        let mut conn = self.pool.get().await.map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::IoError,
                "Pool connection error",
                e.to_string(),
            ))
        })?;
        let json_vec: Vec<Json<T>> =
            conn.srandmember_multiple(&*self.key, count).await?;
        Ok(json_vec.into_iter().map(|json| json.inner()).collect())
    }

    /// Compute the union of multiple sets
    pub async fn union(
        &mut self, other_keys: &[&str],
    ) -> RedisResult<HashSet<T>> {
        let mut conn = self.pool.get().await.map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::IoError,
                "Pool connection error",
                e.to_string(),
            ))
        })?;
        let mut keys = vec![&*self.key];
        keys.extend(other_keys);
        let json_set: HashSet<Json<T>> = conn.sunion(&keys).await?;
        Ok(json_set.into_iter().map(|json| json.inner()).collect())
    }

    /// Compute the intersection of multiple sets
    pub async fn intersect(
        &mut self, other_keys: &[&str],
    ) -> RedisResult<HashSet<T>> {
        let mut conn = self.pool.get().await.map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::IoError,
                "Pool connection error",
                e.to_string(),
            ))
        })?;
        let mut keys = vec![&*self.key];
        keys.extend(other_keys);
        let json_set: HashSet<Json<T>> = conn.sinter(&keys).await?;
        Ok(json_set.into_iter().map(|json| json.inner()).collect())
    }

    /// Compute the difference between sets
    pub async fn diff(
        &mut self, other_keys: &[&str],
    ) -> RedisResult<HashSet<T>> {
        let mut conn = self.pool.get().await.map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::IoError,
                "Pool connection error",
                e.to_string(),
            ))
        })?;
        let mut keys = vec![&*self.key];
        keys.extend(other_keys);
        let json_set: HashSet<Json<T>> = conn.sdiff(&keys).await?;
        Ok(json_set.into_iter().map(|json| json.inner()).collect())
    }

    /// Move member from one set to another
    pub async fn move_to<RV>(
        &mut self, dest_key: &str, value: impl Into<Json<T>>,
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
        conn.smove(&*self.key, dest_key, value.into()).await
    }
}
