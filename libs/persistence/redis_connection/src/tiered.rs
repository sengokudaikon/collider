use std::{borrow::Cow, marker::PhantomData, sync::Arc, time::Duration};

use bytes::Bytes;
use deadpool_redis::redis::{
    AsyncCommands, FromRedisValue, RedisResult, ToRedisArgs, Value,
};
use moka::{future::Cache, notification::RemovalCause};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tracing::instrument;

use super::{
    config::{OverflowStrategy, TieredConfig},
    redis_value::RedisValue,
    type_bind::RedisTypeTrait,
};

struct EvictionMessage {
    key: String,
    value: Bytes,
}

pub struct Tiered<'redis, R, T> {
    redis: &'redis mut R,
    memory: Cache<String, Bytes>,
    key: Cow<'static, str>,
    config: TieredConfig,
    eviction_tx: Option<Sender<EvictionMessage>>,
    __phantom: PhantomData<T>,
}

impl<'redis, R, T> RedisTypeTrait<'redis, R> for Tiered<'redis, R, T> {
    #[instrument(skip(redis, memory), fields(key = %key))]
    fn from_redis_and_key(
        redis: &'redis mut R, key: Cow<'static, str>,
        memory: Option<Cache<String, Bytes>>,
    ) -> Self {
        let config = TieredConfig {
            memory: super::config::MemoryConfig {
                capacity: 10_000,
                ttl_secs: 300,
            },
            overflow_strategy: OverflowStrategy::Drop,
        };

        let (tx, _rx) = mpsc::channel(100);
        let memory = memory.unwrap_or_else(|| {
            let tx = tx.clone();
            Cache::builder()
                .max_capacity(config.memory.capacity)
                .time_to_live(config.memory.ttl())
                .eviction_listener(
                    move |key: Arc<String>,
                          value: Bytes,
                          cause: RemovalCause| {
                        if cause.was_evicted() {
                            return;
                        }
                        let _ = tx.try_send(EvictionMessage {
                            key: key.to_string(),
                            value,
                        });
                    },
                )
                .build()
        });

        Self {
            redis,
            memory,
            key,
            config,
            eviction_tx: Some(tx),
            __phantom: PhantomData,
        }
    }
}

impl<'redis, R, T> Tiered<'redis, R, T>
where
    R: deadpool_redis::redis::aio::ConnectionLike
        + Send
        + Sync
        + Clone
        + 'static,
    T: for<'a> RedisValue<'a> + Send + Sync + 'static,
    for<'a> <T as RedisValue<'a>>::Output:
        ToRedisArgs + Clone + Send + 'static,
{
    pub fn with_config(mut self, config: TieredConfig) -> Self {
        let (tx, rx) = mpsc::channel(100);
        let tx_clone = tx.clone();

        if config.overflow_strategy == OverflowStrategy::MoveToRedis {
            let redis = self.redis.clone();
            tokio::spawn(async move {
                Self::handle_evictions(redis, rx).await;
            });
        }

        self.memory = Cache::builder()
            .max_capacity(config.memory.capacity)
            .time_to_live(config.memory.ttl())
            .eviction_listener(
                move |key: Arc<String>, value: Bytes, cause: RemovalCause| {
                    if cause.was_evicted() {
                        return;
                    }
                    let _ = tx_clone.try_send(EvictionMessage {
                        key: key.to_string(),
                        value,
                    });
                },
            )
            .build();

        self.config = config;
        self.eviction_tx = Some(tx);
        self
    }

    async fn handle_evictions<'a>(
        mut redis: R, mut rx: Receiver<EvictionMessage>,
    ) where
        T: for<'b> RedisValue<'b>,
        for<'b> <T as RedisValue<'b>>::Output: FromRedisValue + ToRedisArgs,
    {
        while let Some(msg) = rx.recv().await {
            if let Ok(value) = <T as RedisValue<'_>>::Output::from_redis_value(
                &Value::BulkString(msg.value.to_vec()),
            ) {
                let _ = redis.set::<_, _, ()>(&msg.key, value).await;
            }
        }
    }

    /// Check if key exists in either cache
    pub async fn exists<RV>(&mut self) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        if self.memory.get(&self.key.to_string()).await.is_some() {
            return FromRedisValue::from_redis_value(&Value::Int(1));
        }
        self.redis.exists(&*self.key).await
    }

    /// Get value, checking memory cache first
    pub async fn get(
        &mut self,
    ) -> RedisResult<<T as RedisValue<'_>>::Output> {
        // Try memory cache first
        if let Some(bytes) = self.memory.get(&self.key.to_string()).await {
            return FromRedisValue::from_redis_value(&Value::BulkString(
                bytes.to_vec(),
            ));
        }

        // Get from Redis
        let value: T::Output = self.redis.get(&*self.key).await?;

        // Store in memory cache
        let mut bytes = Vec::new();
        value.clone().write_redis_args(&mut bytes);
        let bytes: Vec<u8> = bytes.into_iter().flatten().collect();
        self.memory
            .insert(self.key.to_string(), Bytes::from(bytes))
            .await;

        Ok(value)
    }

    /// Try to get value, returns None if not exists
    pub async fn try_get(
        &mut self,
    ) -> RedisResult<Option<<T as RedisValue<'_>>::Output>> {
        if !bool::from_redis_value(&self.exists::<Value>().await?)? {
            return Ok(None);
        }
        self.get().await.map(Some)
    }

    /// Set value in both caches
    pub async fn set<RV>(
        &mut self, value: <T as RedisValue<'_>>::Input,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        let result: RV = self.redis.set(&*self.key, value).await?;

        // Update memory cache with the stored value
        if let Ok(stored_value) =
            self.redis.get::<_, T::Output>(&*self.key).await
        {
            let mut bytes = Vec::new();
            stored_value.write_redis_args(&mut bytes);
            let bytes: Vec<u8> = bytes.into_iter().flatten().collect();
            self.memory
                .insert(self.key.to_string(), Bytes::from(bytes))
                .await;
        }

        Ok(result)
    }

    /// Set value with expiration
    pub async fn set_with_expire<RV>(
        &mut self, value: <T as RedisValue<'_>>::Input, duration: Duration,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        let result: RV = self
            .redis
            .set_ex(&*self.key, value, duration.as_secs() as _)
            .await?;

        // Update memory cache with the stored value
        if let Ok(stored_value) =
            self.redis.get::<_, T::Output>(&*self.key).await
        {
            let mut bytes = Vec::new();
            stored_value.write_redis_args(&mut bytes);
            let bytes: Vec<u8> = bytes.into_iter().flatten().collect();
            self.memory
                .insert(self.key.to_string(), Bytes::from(bytes))
                .await;
        }

        Ok(result)
    }

    /// Set if not exists
    pub async fn set_if_not_exist<RV>(
        &mut self, value: <T as RedisValue<'_>>::Input,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        let result = self.redis.set_nx(&*self.key, value).await;

        // Update memory cache if set was successful
        if let Ok(_val) = &result {
            if let Ok(stored_value) =
                self.redis.get::<_, T::Output>(&*self.key).await
            {
                let mut bytes = Vec::new();
                stored_value.write_redis_args(&mut bytes);
                let bytes: Vec<u8> = bytes.into_iter().flatten().collect();
                self.memory
                    .insert(self.key.to_string(), Bytes::from(bytes))
                    .await;
            }
        }

        result
    }

    /// Remove value from both caches
    pub async fn remove<RV>(&mut self) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.memory.invalidate(&self.key.to_string()).await;
        self.redis.del(&*self.key).await
    }
}

impl<R, T> Drop for Tiered<'_, R, T> {
    fn drop(&mut self) {
        // Ensure we drop the sender so the eviction handler can complete
        self.eviction_tx.take();
    }
}
