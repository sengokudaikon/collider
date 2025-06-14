use std::{borrow::Cow, marker::PhantomData, sync::Arc, time::Duration};

use bytes::Bytes;
use deadpool_redis::redis::{
    AsyncCommands, FromRedisValue, RedisResult, Value,
};
use flume::{Receiver, Sender};
use moka::{future::Cache, notification::RemovalCause};
use tracing::instrument;

use crate::{
    config::{OverflowStrategy, TieredConfig},
    core::{value::{Json, CacheValue}, type_bind::RedisTypeTrait},
};
use serde::{Serialize, Deserialize};

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
            memory: crate::config::MemoryConfig {
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
    T: Serialize + for<'de> Deserialize<'de> + Send + Sync + 'redis,
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
    ) -> RedisResult<T> {
        if let Some(bytes) = self.memory.get(&self.key.to_string()).await {
            let json = Json::<T>::from_bytes(&bytes)
                .map_err(|e| deadpool_redis::redis::RedisError::from((deadpool_redis::redis::ErrorKind::TypeError, "Deserialization failed", e.to_string())))?;
            return Ok(json.inner());
        }

        let json: Json<T> = self.redis.get(&*self.key).await?;
        let value = json.inner();

        // Re-fetch the raw bytes from Redis to store in cache
        if let Ok(json_value) = self.redis.get::<_, Json<T>>(&*self.key).await {
            if let Ok(serialized) = json_value.to_bytes() {
                self.memory
                    .insert(self.key.to_string(), Bytes::from(serialized))
                    .await;
            }
        }

        Ok(value)
    }

    pub async fn try_get(
        &mut self,
    ) -> RedisResult<Option<T>> {
        if !bool::from_redis_value(&self.exists::<Value>().await?)? {
            return Ok(None);
        }
        self.get().await.map(Some)
    }

    pub async fn set<RV>(
        &mut self, value: impl Into<Json<T>>,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        let json = value.into();
        let result: RV = self.redis.set(&*self.key, &json).await?;

        // Store in memory cache as well
        if let Ok(serialized) = json.to_bytes() {
            self.memory
                .insert(self.key.to_string(), Bytes::from(serialized))
                .await;
        }

        Ok(result)
    }

    pub async fn set_with_expire<RV>(
        &mut self, value: impl Into<Json<T>>, duration: Duration,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        let json = value.into();
        let result: RV = self
            .redis
            .set_ex(&*self.key, &json, duration.as_secs() as _)
            .await?;

        // Store in memory cache as well
        if let Ok(serialized) = json.to_bytes() {
            self.memory
                .insert(self.key.to_string(), Bytes::from(serialized))
                .await;
        }

        Ok(result)
    }

    pub async fn set_if_not_exist<RV>(
        &mut self, value: impl Into<Json<T>>,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        let json = value.into();
        let result = self.redis.set_nx(&*self.key, &json).await;

        if let Ok(_val) = &result {
            // Store in memory cache as well
            if let Ok(serialized) = json.to_bytes() {
                self.memory
                    .insert(self.key.to_string(), Bytes::from(serialized))
                    .await;
            }
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
