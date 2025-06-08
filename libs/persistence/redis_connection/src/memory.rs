use std::{borrow::Cow, marker::PhantomData, time::Duration};

use bytes::Bytes;
use deadpool_redis::redis::{
    FromRedisValue, RedisResult, ToRedisArgs, Value,
};
use moka::future::Cache;

use super::{
    config::MemoryConfig, redis_value::RedisValue, type_bind::RedisTypeTrait,
};

pub struct Memory<'redis, R, T> {
    memory: Cache<String, Bytes>,
    key: Cow<'static, str>,
    config: MemoryConfig,
    __phantom: PhantomData<(&'redis R, T)>,
}

impl<'redis, R, T> RedisTypeTrait<'redis, R> for Memory<'redis, R, T> {
    fn from_redis_and_key(
        _redis: &'redis mut R, key: Cow<'static, str>,
        memory: Option<Cache<String, Bytes>>,
    ) -> Self {
        let config = MemoryConfig {
            capacity: 10_000,
            ttl_secs: 300,
        };

        let memory = memory.unwrap_or_else(|| {
            Cache::builder()
                .max_capacity(config.capacity)
                .time_to_live(config.ttl())
                .build()
        });

        Self {
            memory,
            key,
            config,
            __phantom: PhantomData,
        }
    }
}

impl<'redis, R, T> Memory<'redis, R, T>
where
    T: RedisValue<'redis>,
    T::Output: ToRedisArgs + Clone,
{
    pub fn with_config(mut self, config: MemoryConfig) -> Self {
        self.memory = Cache::builder()
            .max_capacity(config.capacity)
            .time_to_live(config.ttl())
            .build();
        self.config = config;
        self
    }

    pub async fn exists<RV>(&mut self) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        let exists = self.memory.get(&self.key.to_string()).await.is_some();
        FromRedisValue::from_redis_value(&Value::Int(exists as i64))
    }

    pub async fn get(&mut self) -> RedisResult<T::Output> {
        if let Some(bytes) = self.memory.get(&self.key.to_string()).await {
            FromRedisValue::from_redis_value(&Value::BulkString(
                bytes.to_vec(),
            ))
        }
        else {
            Err(deadpool_redis::redis::RedisError::from((
                deadpool_redis::redis::ErrorKind::TypeError,
                "Key not found in memory cache",
            )))
        }
    }

    pub async fn try_get(&mut self) -> RedisResult<Option<T::Output>> {
        if !bool::from_redis_value(&self.exists::<Value>().await?)? {
            return Ok(None);
        }
        self.get().await.map(Some)
    }

    pub async fn set<RV>(&mut self, value: T::Input) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        let mut bytes = Vec::new();
        value.write_redis_args(&mut bytes);
        let flat_bytes: Vec<u8> = bytes.into_iter().flatten().collect();
        self.memory
            .insert(self.key.to_string(), Bytes::from(flat_bytes))
            .await;
        FromRedisValue::from_redis_value(&Value::SimpleString(
            "OK".to_string(),
        ))
    }

    pub async fn set_with_expire<RV>(
        &mut self, value: T::Input, _duration: Duration,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.set(value).await
    }

    pub async fn set_if_not_exist<RV>(
        &mut self, value: T::Input,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        let exists = self.memory.get(&self.key.to_string()).await.is_some();
        if !exists {
            let mut bytes = Vec::new();
            value.write_redis_args(&mut bytes);
            let flat_bytes: Vec<u8> = bytes.into_iter().flatten().collect();
            self.memory
                .insert(self.key.to_string(), Bytes::from(flat_bytes))
                .await;
            FromRedisValue::from_redis_value(&Value::Int(1))
        }
        else {
            FromRedisValue::from_redis_value(&Value::Int(0))
        }
    }

    pub async fn remove<RV>(&mut self) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        let existed = self.memory.get(&self.key.to_string()).await.is_some();
        self.memory.invalidate(&self.key.to_string()).await;
        FromRedisValue::from_redis_value(&Value::Int(existed as i64))
    }
}
