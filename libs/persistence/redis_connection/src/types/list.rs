#![allow(unused)]
use std::{borrow::Cow, marker::PhantomData, time::Duration};

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

pub struct List<T> {
    pool: deadpool_redis::Pool,
    key: Cow<'static, str>,
    __phantom: PhantomData<T>,
}

impl<T> CacheTypeTrait<'_> for List<T> {
    fn from_cache_and_key(
        backend: CacheBackend<'_>, key: Cow<'static, str>,
    ) -> Self {
        let pool = match backend {
            CacheBackend::Redis(pool) => pool,
            _ => panic!("List type can only be created from Redis backend"),
        };

        Self {
            pool,
            key,
            __phantom: PhantomData,
        }
    }
}

impl<T> List<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone,
{
    /// Push element to the left (beginning) of the list
    pub async fn push_left<RV>(
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
        conn.lpush(&*self.key, value.into()).await
    }

    /// Push multiple elements to the left of the list
    pub async fn push_left_multiple<RV>(
        &mut self, values: Vec<T>,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        // Redis doesn't have lpush_multiple, so we use multiple lpush calls
        let mut conn = self.pool.get().await.map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::IoError,
                "Pool connection error",
                e.to_string(),
            ))
        })?;
        let mut result: i32 = 0;
        for value in values {
            result = conn.lpush(&*self.key, Json(value)).await?;
        }
        FromRedisValue::from_redis_value(&redis::Value::Int(result as i64))
    }

    /// Push element to the right (end) of the list
    pub async fn push_right<RV>(
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
        conn.rpush(&*self.key, value.into()).await
    }

    /// Push multiple elements to the right of the list
    pub async fn push_right_multiple<RV>(
        &mut self, values: Vec<T>,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        // Redis doesn't have rpush_multiple, so we use multiple rpush calls
        let mut conn = self.pool.get().await.map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::IoError,
                "Pool connection error",
                e.to_string(),
            ))
        })?;
        let mut result: i32 = 0;
        for value in values {
            result = conn.rpush(&*self.key, Json(value)).await?;
        }
        FromRedisValue::from_redis_value(&redis::Value::Int(result as i64))
    }

    /// Pop element from the left (beginning) of the list
    pub async fn pop_left(&mut self) -> RedisResult<Option<T>> {
        let mut conn = self.pool.get().await.map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::IoError,
                "Pool connection error",
                e.to_string(),
            ))
        })?;
        let json: Option<Json<T>> = conn.lpop(&*self.key, None).await?;
        Ok(json.map(|j| j.inner()))
    }

    /// Pop element from the right (end) of the list
    pub async fn pop_right(&mut self) -> RedisResult<Option<T>> {
        let mut conn = self.pool.get().await.map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::IoError,
                "Pool connection error",
                e.to_string(),
            ))
        })?;
        let json: Option<Json<T>> = conn.rpop(&*self.key, None).await?;
        Ok(json.map(|j| j.inner()))
    }

    /// Pop multiple elements from the left
    pub async fn pop_left_multiple(
        &mut self, count: usize,
    ) -> RedisResult<Vec<T>> {
        if let Some(non_zero_count) = std::num::NonZero::new(count) {
            let mut conn = self.pool.get().await.map_err(|e| {
                redis::RedisError::from((
                    redis::ErrorKind::IoError,
                    "Pool connection error",
                    e.to_string(),
                ))
            })?;
            let jsons: Vec<Json<T>> =
                conn.lpop(&*self.key, Some(non_zero_count)).await?;
            Ok(jsons.into_iter().map(|j| j.inner()).collect())
        }
        else {
            Ok(vec![])
        }
    }

    /// Pop multiple elements from the right
    pub async fn pop_right_multiple(
        &mut self, count: usize,
    ) -> RedisResult<Vec<T>> {
        if let Some(non_zero_count) = std::num::NonZero::new(count) {
            let mut conn = self.pool.get().await.map_err(|e| {
                redis::RedisError::from((
                    redis::ErrorKind::IoError,
                    "Pool connection error",
                    e.to_string(),
                ))
            })?;
            let jsons: Vec<Json<T>> =
                conn.rpop(&*self.key, Some(non_zero_count)).await?;
            Ok(jsons.into_iter().map(|j| j.inner()).collect())
        }
        else {
            Ok(vec![])
        }
    }

    /// Blocking pop from left with timeout
    pub async fn blocking_pop_left(
        &mut self, timeout: Duration,
    ) -> RedisResult<Option<(String, T)>> {
        let mut conn = self.pool.get().await.map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::IoError,
                "Pool connection error",
                e.to_string(),
            ))
        })?;
        let result: Option<(String, Json<T>)> =
            conn.blpop(&*self.key, timeout.as_secs() as f64).await?;
        Ok(result.map(|(k, v)| (k, v.inner())))
    }

    /// Blocking pop from right with timeout
    pub async fn blocking_pop_right(
        &mut self, timeout: Duration,
    ) -> RedisResult<Option<(String, T)>> {
        let mut conn = self.pool.get().await.map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::IoError,
                "Pool connection error",
                e.to_string(),
            ))
        })?;
        let result: Option<(String, Json<T>)> =
            conn.brpop(&*self.key, timeout.as_secs() as f64).await?;
        Ok(result.map(|(k, v)| (k, v.inner())))
    }

    /// Get element at index
    pub async fn get(&mut self, index: isize) -> RedisResult<Option<T>> {
        let mut conn = self.pool.get().await.map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::IoError,
                "Pool connection error",
                e.to_string(),
            ))
        })?;
        let json: Option<Json<T>> = conn.lindex(&*self.key, index).await?;
        Ok(json.map(|j| j.inner()))
    }

    /// Set element at index
    pub async fn set<RV>(
        &mut self, index: isize, value: impl Into<Json<T>>,
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
        conn.lset(&*self.key, index, value.into()).await
    }

    /// Get range of elements
    pub async fn range(
        &mut self, start: isize, stop: isize,
    ) -> RedisResult<Vec<T>> {
        let mut conn = self.pool.get().await.map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::IoError,
                "Pool connection error",
                e.to_string(),
            ))
        })?;
        let jsons: Vec<Json<T>> =
            conn.lrange(&*self.key, start, stop).await?;
        Ok(jsons.into_iter().map(|j| j.inner()).collect())
    }

    /// Get all elements in the list
    pub async fn all(&mut self) -> RedisResult<Vec<T>> {
        self.range(0, -1).await
    }

    /// Get length of the list
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
        conn.llen(&*self.key).await
    }

    /// Insert element before or after pivot
    pub async fn insert<RV>(
        &mut self, before: bool, pivot: impl Into<Json<T>>,
        value: impl Into<Json<T>>,
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
        let pivot_json = pivot.into();
        let value_json = value.into();
        if before {
            conn.linsert_before(&*self.key, pivot_json, value_json)
                .await
        }
        else {
            conn.linsert_after(&*self.key, pivot_json, value_json).await
        }
    }

    /// Remove occurrences of element
    pub async fn remove<RV>(
        &mut self, count: isize, value: impl Into<Json<T>>,
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
        conn.lrem(&*self.key, count, value.into()).await
    }

    /// Trim list to specified range
    pub async fn trim<RV>(
        &mut self, start: isize, stop: isize,
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
        conn.ltrim(&*self.key, start, stop).await
    }

    /// Move element from one list to another (non-blocking version)
    pub async fn move_to(
        &mut self, dest_key: &str, from_left: bool, to_left: bool,
    ) -> RedisResult<Option<T>> {
        // Use the simpler RPOPLPUSH or equivalent operations
        let mut conn = self.pool.get().await.map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::IoError,
                "Pool connection error",
                e.to_string(),
            ))
        })?;
        if from_left {
            let value: Option<T> = self.pop_left().await?;
            if let Some(val) = value.clone() {
                if to_left {
                    let _: i32 = conn.lpush(dest_key, Json(val)).await?;
                }
                else {
                    let _: i32 = conn.rpush(dest_key, Json(val)).await?;
                }
                Ok(value)
            }
            else {
                Ok(None)
            }
        }
        else {
            let value: Option<T> = self.pop_right().await?;
            if let Some(val) = value.clone() {
                if to_left {
                    let _: i32 = conn.lpush(dest_key, Json(val)).await?;
                }
                else {
                    let _: i32 = conn.rpush(dest_key, Json(val)).await?;
                }
                Ok(value)
            }
            else {
                Ok(None)
            }
        }
    }

    /// Push element only if list exists
    pub async fn push_left_if_exists<RV>(
        &mut self, value: impl Into<Json<T>>,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        // Check if list exists first, then push
        let mut conn = self.pool.get().await.map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::IoError,
                "Pool connection error",
                e.to_string(),
            ))
        })?;
        let exists: bool = conn.exists(&*self.key).await?;
        if exists {
            conn.lpush(&*self.key, value.into()).await
        }
        else {
            FromRedisValue::from_redis_value(&redis::Value::Int(0))
        }
    }

    /// Push element only if list exists
    pub async fn push_right_if_exists<RV>(
        &mut self, value: impl Into<Json<T>>,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        // Check if list exists first, then push
        let mut conn = self.pool.get().await.map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::IoError,
                "Pool connection error",
                e.to_string(),
            ))
        })?;
        let exists: bool = conn.exists(&*self.key).await?;
        if exists {
            conn.rpush(&*self.key, value.into()).await
        }
        else {
            FromRedisValue::from_redis_value(&redis::Value::Int(0))
        }
    }
}
