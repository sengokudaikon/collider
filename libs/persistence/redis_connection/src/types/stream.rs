#![allow(unused)]
use std::{
    borrow::Cow, collections::HashMap, marker::PhantomData, time::Duration,
};

use bytes::Bytes;
use deadpool_redis::redis::{
    AsyncCommands, FromRedisValue, RedisResult, ToRedisArgs,
    streams::{
        StreamId, StreamRangeReply, StreamReadOptions, StreamReadReply,
    },
};
use moka::future::Cache;
use serde::{Deserialize, Serialize};

use crate::core::{
    backend::CacheBackend,
    type_bind::CacheTypeTrait,
    value::{CacheValue, Json},
};

pub struct Stream<'cache, T> {
    redis: &'cache mut deadpool_redis::Connection,
    key: Cow<'static, str>,
    __phantom: PhantomData<T>,
}

impl<'cache, T> CacheTypeTrait<'cache> for Stream<'cache, T> {
    fn from_cache_and_key(
        backend: CacheBackend<'cache>, key: Cow<'static, str>,
    ) -> Self {
        let redis = match backend {
            CacheBackend::Redis(redis) => redis,
            _ => panic!("Stream type can only be created from Redis backend"),
        };

        Self {
            redis,
            key,
            __phantom: PhantomData,
        }
    }
}

impl<'cache, T> Stream<'cache, T>
where
    T: Serialize + for<'de> Deserialize<'de> + Send + Sync + 'cache,
{
    /// Add entry to stream with auto-generated ID
    pub async fn add_auto<F>(
        &mut self, fields: &[(&str, F)],
    ) -> RedisResult<String>
    where
        F: Into<Json<T>> + Clone,
    {
        let json_fields: Vec<(&str, Json<T>)> =
            fields.iter().map(|(k, v)| (*k, v.clone().into())).collect();
        self.redis.xadd(&*self.key, "*", &json_fields).await
    }

    /// Add entry to stream with specific ID
    pub async fn add_with_id<F>(
        &mut self, id: &str, fields: &[(&str, F)],
    ) -> RedisResult<String>
    where
        F: Into<Json<T>> + Clone,
    {
        let json_fields: Vec<(&str, Json<T>)> =
            fields.iter().map(|(k, v)| (*k, v.clone().into())).collect();
        self.redis.xadd(&*self.key, id, &json_fields).await
    }

    /// Add entry with maximum length constraint
    pub async fn add_with_maxlen<F>(
        &mut self, maxlen: usize, fields: &[(&str, F)],
    ) -> RedisResult<String>
    where
        F: Into<Json<T>> + Clone,
    {
        let json_fields: Vec<(&str, Json<T>)> =
            fields.iter().map(|(k, v)| (*k, v.clone().into())).collect();
        self.redis
            .xadd_maxlen(
                &*self.key,
                redis::streams::StreamMaxlen::Equals(maxlen),
                "*",
                &json_fields,
            )
            .await
    }

    /// Get length of stream
    pub async fn len<RV>(&mut self) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.redis.xlen(&*self.key).await
    }

    /// Read entries from stream by range
    pub async fn range(
        &mut self, start: &str, end: &str,
    ) -> RedisResult<StreamRangeReply> {
        self.redis.xrange(&*self.key, start, end).await
    }

    /// Read entries from stream by range with count limit
    pub async fn range_count(
        &mut self, start: &str, end: &str, count: usize,
    ) -> RedisResult<StreamRangeReply> {
        self.redis.xrange_count(&*self.key, start, end, count).await
    }

    /// Read entries in reverse order
    pub async fn reverse_range(
        &mut self, end: &str, start: &str,
    ) -> RedisResult<StreamRangeReply> {
        self.redis.xrevrange(&*self.key, end, start).await
    }

    /// Read entries in reverse order with count limit
    pub async fn reverse_range_count(
        &mut self, end: &str, start: &str, count: usize,
    ) -> RedisResult<StreamRangeReply> {
        self.redis
            .xrevrange_count(&*self.key, end, start, count)
            .await
    }

    /// Read new entries from stream (blocking)
    pub async fn read(&mut self, id: &str) -> RedisResult<StreamReadReply> {
        let opts = StreamReadOptions::default().count(10);
        self.redis.xread_options(&[&*self.key], &[id], &opts).await
    }

    /// Read new entries with blocking timeout
    pub async fn read_blocking(
        &mut self, id: &str, timeout: Duration,
    ) -> RedisResult<StreamReadReply> {
        let opts = StreamReadOptions::default()
            .count(10)
            .block(timeout.as_millis() as usize);
        self.redis.xread_options(&[&*self.key], &[id], &opts).await
    }

    /// Delete entries from stream
    pub async fn delete<RV>(&mut self, ids: &[&str]) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.redis.xdel(&*self.key, ids).await
    }

    /// Trim stream to maximum length
    pub async fn trim<RV>(&mut self, maxlen: usize) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.redis
            .xtrim(&*self.key, redis::streams::StreamMaxlen::Equals(maxlen))
            .await
    }

    /// Trim stream approximately to maximum length (more efficient)
    pub async fn trim_approx<RV>(&mut self, maxlen: usize) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.redis
            .xtrim(&*self.key, redis::streams::StreamMaxlen::Approx(maxlen))
            .await
    }

    /// Create consumer group
    pub async fn create_group<RV>(
        &mut self, group: &str, id: &str,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.redis.xgroup_create(&*self.key, group, id).await
    }

    /// Create consumer group with MKSTREAM option
    pub async fn create_group_mkstream<RV>(
        &mut self, group: &str, id: &str,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.redis
            .xgroup_create_mkstream(&*self.key, group, id)
            .await
    }

    /// Delete consumer group
    pub async fn delete_group<RV>(&mut self, group: &str) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.redis.xgroup_destroy(&*self.key, group).await
    }

    /// Read from consumer group
    pub async fn read_group(
        &mut self, group: &str, consumer: &str, id: &str,
    ) -> RedisResult<StreamReadReply> {
        let opts = StreamReadOptions::default()
            .count(10)
            .group(group, consumer);
        self.redis.xread_options(&[&*self.key], &[id], &opts).await
    }

    /// Read from consumer group with blocking
    pub async fn read_group_blocking(
        &mut self, group: &str, consumer: &str, id: &str, timeout: Duration,
    ) -> RedisResult<StreamReadReply> {
        let opts = StreamReadOptions::default()
            .count(10)
            .block(timeout.as_millis() as usize)
            .group(group, consumer);
        self.redis.xread_options(&[&*self.key], &[id], &opts).await
    }

    /// Acknowledge message processing
    pub async fn ack<RV>(
        &mut self, group: &str, ids: &[&str],
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.redis.xack(&*self.key, group, ids).await
    }

    /// Get pending messages info
    pub async fn pending(
        &mut self, group: &str,
    ) -> RedisResult<redis::streams::StreamPendingReply> {
        self.redis.xpending(&*self.key, group).await
    }

    /// Get detailed pending messages
    pub async fn pending_count(
        &mut self, group: &str, start: &str, end: &str, count: usize,
    ) -> RedisResult<redis::streams::StreamPendingCountReply> {
        self.redis
            .xpending_count(&*self.key, group, start, end, count)
            .await
    }

    /// Claim pending messages
    pub async fn claim(
        &mut self, group: &str, consumer: &str, min_idle_time: usize,
        ids: &[&str],
    ) -> RedisResult<StreamRangeReply> {
        self.redis
            .xclaim(&*self.key, group, consumer, min_idle_time, ids)
            .await
    }

    /// Auto-claim pending messages (simplified to use xclaim)
    pub async fn auto_claim(
        &mut self, group: &str, consumer: &str, min_idle_time: usize,
        ids: &[&str],
    ) -> RedisResult<StreamRangeReply> {
        self.redis
            .xclaim(&*self.key, group, consumer, min_idle_time, ids)
            .await
    }

    /// Get stream info
    pub async fn info(
        &mut self,
    ) -> RedisResult<redis::streams::StreamInfoStreamReply> {
        self.redis.xinfo_stream(&*self.key).await
    }

    /// Get consumer groups info
    pub async fn info_groups(
        &mut self,
    ) -> RedisResult<redis::streams::StreamInfoGroupsReply> {
        self.redis.xinfo_groups(&*self.key).await
    }

    /// Get consumers info for a group
    pub async fn info_consumers(
        &mut self, group: &str,
    ) -> RedisResult<redis::streams::StreamInfoConsumersReply> {
        self.redis.xinfo_consumers(&*self.key, group).await
    }
}
