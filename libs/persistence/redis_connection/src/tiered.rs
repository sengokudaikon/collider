use std::{borrow::Cow, marker::PhantomData, sync::Arc, time::Duration};

use bytes::Bytes;
use deadpool_redis::redis::{
    AsyncCommands, FromRedisValue, RedisResult, ToRedisArgs, Value,
};
use flume::{Receiver, Sender};
use moka::{future::Cache, notification::RemovalCause};
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

// Standalone eviction handler function
async fn handle_evictions(rx: Receiver<EvictionMessage>) {
    use crate::connection::RedisConnectionManager;

    while let Ok(msg) = rx.recv_async().await {
        // Get a fresh connection from the pool for eviction handling
        if let Ok(redis) =
            RedisConnectionManager::from_static().get_connection().await
        {
            let mut redis = redis;
            // Store raw bytes back to Redis when evicted from memory
            let _ = redis.set::<_, _, ()>(&msg.key, msg.value.as_ref()).await;
        }
    }
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
            overflow_strategy: OverflowStrategy::MoveToRedis, /* Enable eviction to Redis */
        };

        let (tx, rx) = flume::unbounded();

        // Start eviction handler if configured
        if config.overflow_strategy == OverflowStrategy::MoveToRedis {
            tokio::spawn(async move {
                handle_evictions(rx).await;
            });
        }

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
    R: deadpool_redis::redis::aio::ConnectionLike + Send + Sync + 'static,
    T: for<'a> RedisValue<'a> + Send + Sync + 'static,
    for<'a> <T as RedisValue<'a>>::Input:
        ToRedisArgs + Clone + Send + 'static,
    for<'a> <T as RedisValue<'a>>::Output: Clone + Send + 'static,
{
    pub fn with_config(mut self, config: TieredConfig) -> Self {
        let (tx, rx) = flume::unbounded();
        let tx_clone = tx.clone();

        if config.overflow_strategy == OverflowStrategy::MoveToRedis {
            tokio::spawn(async move {
                handle_evictions(rx).await;
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

    pub async fn exists<RV>(&mut self) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        if self.memory.get(&self.key.to_string()).await.is_some() {
            return FromRedisValue::from_redis_value(&Value::Int(1));
        }
        self.redis.exists(&*self.key).await
    }

    pub async fn get(
        &mut self,
    ) -> RedisResult<<T as RedisValue<'_>>::Output> {
        if let Some(bytes) = self.memory.get(&self.key.to_string()).await {
            return FromRedisValue::from_redis_value(&Value::BulkString(
                bytes.to_vec(),
            ));
        }

        let value: T::Output = self.redis.get(&*self.key).await?;

        // Store the raw Redis bytes in memory cache
        if let Ok(raw_bytes) = self.redis.get::<_, Vec<u8>>(&*self.key).await
        {
            self.memory
                .insert(self.key.to_string(), Bytes::from(raw_bytes))
                .await;
        }

        Ok(value)
    }

    pub async fn try_get(
        &mut self,
    ) -> RedisResult<Option<<T as RedisValue<'_>>::Output>> {
        if !bool::from_redis_value(&self.exists::<Value>().await?)? {
            return Ok(None);
        }
        self.get().await.map(Some)
    }

    pub async fn set<RV>(
        &mut self, value: <T as RedisValue<'_>>::Input,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        let result: RV = self.redis.set(&*self.key, value).await?;

        // Cache will be populated on next get from Redis

        Ok(result)
    }

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

        // Cache will be populated on next get from Redis

        Ok(result)
    }

    pub async fn set_if_not_exist<RV>(
        &mut self, value: <T as RedisValue<'_>>::Input,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        let result = self.redis.set_nx(&*self.key, value).await;

        if let Ok(_val) = &result {
            // Cache will be populated on next get from Redis
        }

        result
    }

    pub async fn remove<RV>(&mut self) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        self.memory.invalidate(&self.key.to_string()).await;
        self.redis.del(&*self.key).await
    }
}

impl<R, T> Drop for Tiered<'_, R, T> {
    fn drop(&mut self) { self.eviction_tx.take(); }
}
