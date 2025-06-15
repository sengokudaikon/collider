use std::{borrow::Cow, marker::PhantomData, time::Duration};

use async_trait::async_trait;
use deadpool_redis::Connection;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};

use super::r#trait::{CacheError, CacheResult, CacheTrait};
use crate::core::{type_bind::CacheTypeTrait, value::Json};

/// Redis cache implementation using deadpool Redis connection
pub struct RedisCache<'cache, T> {
    redis: &'cache mut Connection,
    #[allow(dead_code)]
    key: Cow<'static, str>,
    __phantom: PhantomData<T>,
}

impl<'cache, T> CacheTypeTrait<'cache> for RedisCache<'cache, T> {
    fn from_cache_and_key(
        backend: super::super::core::backend::CacheBackend<'cache>,
        key: Cow<'static, str>,
    ) -> Self {
        let redis = match backend {
            super::super::core::backend::CacheBackend::Redis(redis) => redis,
            _ => panic!("RedisCache can only be created from Redis backend"),
        };

        Self {
            redis,
            key,
            __phantom: PhantomData,
        }
    }
}

/// Implement the backend-agnostic CacheTrait for Redis cache operations
#[async_trait]
impl<T> CacheTrait for RedisCache<'_, T>
where
    T: Serialize + for<'de> Deserialize<'de> + Clone + Send + Sync + 'static,
{
    type Value = T;

    async fn exists(&mut self, key: &str) -> CacheResult<bool> {
        self.redis
            .exists(key)
            .await
            .map_err(|e| CacheError::Other(e.to_string()))
    }

    async fn get(&mut self, key: &str) -> CacheResult<Self::Value> {
        let json: Json<T> = self.redis.get(key).await.map_err(|e| {
            match e.kind() {
                redis::ErrorKind::TypeError => CacheError::KeyNotFound,
                _ => CacheError::Other(e.to_string()),
            }
        })?;
        Ok(json.inner())
    }

    async fn try_get(
        &mut self, key: &str,
    ) -> CacheResult<Option<Self::Value>> {
        match self.get(key).await {
            Ok(value) => Ok(Some(value)),
            Err(CacheError::KeyNotFound) => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn set(
        &mut self, key: &str, value: &Self::Value,
    ) -> CacheResult<()> {
        let json = Json(value.clone());
        let _: () = self
            .redis
            .set(key, json)
            .await
            .map_err(|e| CacheError::Other(e.to_string()))?;
        Ok(())
    }

    async fn set_with_ttl(
        &mut self, key: &str, value: &Self::Value, ttl: Duration,
    ) -> CacheResult<()> {
        let json = Json(value.clone());
        let _: () = self
            .redis
            .set_ex(key, json, ttl.as_secs() as _)
            .await
            .map_err(|e| CacheError::Other(e.to_string()))?;
        Ok(())
    }

    async fn set_if_not_exist(
        &mut self, key: &str, value: &Self::Value,
    ) -> CacheResult<bool> {
        let json = Json(value.clone());
        let result: bool = self
            .redis
            .set_nx(key, json)
            .await
            .map_err(|e| CacheError::Other(e.to_string()))?;
        Ok(result)
    }

    async fn remove(&mut self, key: &str) -> CacheResult<bool> {
        let count: u32 = self
            .redis
            .del(key)
            .await
            .map_err(|e| CacheError::Other(e.to_string()))?;
        Ok(count > 0)
    }
}
