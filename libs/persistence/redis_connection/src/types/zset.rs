#![allow(unused)]
use std::{borrow::Cow, marker::PhantomData};

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

pub struct SortedSet<'cache, T> {
    redis: &'cache mut deadpool_redis::Connection,
    key: Cow<'static, str>,
    __phantom: PhantomData<T>,
}

impl<'cache, T> CacheTypeTrait<'cache> for SortedSet<'cache, T> {
    fn from_cache_and_key(
        backend: CacheBackend<'cache>, key: Cow<'static, str>,
    ) -> Self {
        let redis = match backend {
            CacheBackend::Redis(redis) => redis,
            _ => {
                panic!(
                    "SortedSet type can only be created from Redis backend"
                )
            }
        };

        Self {
            redis,
            key,
            __phantom: PhantomData,
        }
    }
}

impl<'cache, T> SortedSet<'cache, T>
where
    T: Serialize + for<'de> Deserialize<'de> + Send + Sync + 'cache,
{
    /// Add member with score to sorted set
    pub async fn add_with_score<RV>(
        &mut self, score: f64, value: impl Into<Json<T>>,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.redis.zadd(&*self.key, value.into(), score).await
    }

    /// Add multiple members with scores
    pub async fn add_multiple<RV>(
        &mut self, items: Vec<(f64, impl Into<Json<T>>)>,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        // Convert items to Json
        let json_items: Vec<(f64, Json<T>)> = items
            .into_iter()
            .map(|(score, value)| (score, value.into()))
            .collect();
        self.redis.zadd_multiple(&*self.key, &json_items).await
    }

    /// Remove member from sorted set
    pub async fn remove<RV>(
        &mut self, value: impl Into<Json<T>>,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.redis.zrem(&*self.key, value.into()).await
    }

    /// Get score of member
    pub async fn score(
        &mut self, value: impl Into<Json<T>>,
    ) -> RedisResult<Option<f64>> {
        self.redis.zscore(&*self.key, value.into()).await
    }

    /// Get rank of member (0-based, lowest score first)
    pub async fn rank(
        &mut self, value: impl Into<Json<T>>,
    ) -> RedisResult<Option<usize>> {
        self.redis.zrank(&*self.key, value.into()).await
    }

    /// Get reverse rank of member (0-based, highest score first)
    pub async fn reverse_rank(
        &mut self, value: impl Into<Json<T>>,
    ) -> RedisResult<Option<usize>> {
        self.redis.zrevrank(&*self.key, value.into()).await
    }

    /// Get number of members in sorted set
    pub async fn len<RV>(&mut self) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.redis.zcard(&*self.key).await
    }

    /// Get members by rank range (0-based)
    pub async fn range(
        &mut self, start: isize, stop: isize,
    ) -> RedisResult<Vec<T>> {
        let json_vec: Vec<Json<T>> =
            self.redis.zrange(&*self.key, start, stop).await?;
        Ok(json_vec.into_iter().map(|json| json.inner()).collect())
    }

    /// Get members by rank range with scores
    pub async fn range_with_scores(
        &mut self, start: isize, stop: isize,
    ) -> RedisResult<Vec<(T, f64)>> {
        let json_vec: Vec<(Json<T>, f64)> = self
            .redis
            .zrange_withscores(&*self.key, start, stop)
            .await?;
        Ok(json_vec
            .into_iter()
            .map(|(json, score)| (json.inner(), score))
            .collect())
    }

    /// Get members by reverse rank range (highest score first)
    pub async fn reverse_range(
        &mut self, start: isize, stop: isize,
    ) -> RedisResult<Vec<T>> {
        let json_vec: Vec<Json<T>> =
            self.redis.zrevrange(&*self.key, start, stop).await?;
        Ok(json_vec.into_iter().map(|json| json.inner()).collect())
    }

    /// Get members by reverse rank range with scores
    pub async fn reverse_range_with_scores(
        &mut self, start: isize, stop: isize,
    ) -> RedisResult<Vec<(T, f64)>> {
        let json_vec: Vec<(Json<T>, f64)> = self
            .redis
            .zrevrange_withscores(&*self.key, start, stop)
            .await?;
        Ok(json_vec
            .into_iter()
            .map(|(json, score)| (json.inner(), score))
            .collect())
    }

    /// Get members by score range
    pub async fn range_by_score(
        &mut self, min: f64, max: f64,
    ) -> RedisResult<Vec<T>> {
        let json_vec: Vec<Json<T>> =
            self.redis.zrangebyscore(&*self.key, min, max).await?;
        Ok(json_vec.into_iter().map(|json| json.inner()).collect())
    }

    /// Get members by score range with scores
    pub async fn range_by_score_with_scores(
        &mut self, min: f64, max: f64,
    ) -> RedisResult<Vec<(T, f64)>> {
        let json_vec: Vec<(Json<T>, f64)> = self
            .redis
            .zrangebyscore_withscores(&*self.key, min, max)
            .await?;
        Ok(json_vec
            .into_iter()
            .map(|(json, score)| (json.inner(), score))
            .collect())
    }

    /// Get members by score range with limit
    pub async fn range_by_score_limit(
        &mut self, min: f64, max: f64, offset: isize, count: isize,
    ) -> RedisResult<Vec<T>> {
        let json_vec: Vec<Json<T>> = self
            .redis
            .zrangebyscore_limit(&*self.key, min, max, offset, count)
            .await?;
        Ok(json_vec.into_iter().map(|json| json.inner()).collect())
    }

    /// Count members in score range
    pub async fn count_by_score<RV>(
        &mut self, min: f64, max: f64,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.redis.zcount(&*self.key, min, max).await
    }

    /// Increment score of member
    pub async fn increment_score(
        &mut self, value: impl Into<Json<T>>, increment: f64,
    ) -> RedisResult<f64> {
        self.redis.zincr(&*self.key, value.into(), increment).await
    }

    /// Remove members by rank range
    pub async fn remove_by_rank<RV>(
        &mut self, start: isize, stop: isize,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.redis.zremrangebyrank(&*self.key, start, stop).await
    }

    /// Remove members by score range (simplified implementation)
    pub async fn remove_by_score<RV>(
        &mut self, min: f64, max: f64,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        // Get members in range and remove them individually
        let members: Vec<T> = self.range_by_score(min, max).await?;
        let mut removed_count = 0u32;
        for member in members {
            let count: u32 =
                self.redis.zrem(&*self.key, Json(member)).await?;
            removed_count += count;
        }
        FromRedisValue::from_redis_value(&redis::Value::Int(
            removed_count as i64,
        ))
    }

    /// Get top N members with highest scores
    pub async fn top(&mut self, count: usize) -> RedisResult<Vec<(T, f64)>> {
        self.reverse_range_with_scores(0, count as isize - 1).await
    }

    /// Get bottom N members with lowest scores
    pub async fn bottom(
        &mut self, count: usize,
    ) -> RedisResult<Vec<(T, f64)>> {
        self.range_with_scores(0, count as isize - 1).await
    }
}
