use std::{borrow::Cow, marker::PhantomData, time::Duration};

use async_trait::async_trait;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};

use super::r#trait::{CacheError, CacheResult, CacheTrait};
use crate::core::{type_bind::CacheTypeTrait, value::Json};

/// Redis cache implementation using deadpool Redis pool
pub struct RedisCache<T> {
    pool: deadpool_redis::Pool,
    #[allow(dead_code)]
    key: Cow<'static, str>,
    __phantom: PhantomData<T>,
}

impl<T> CacheTypeTrait<'_> for RedisCache<T> {
    fn from_cache_and_key(
        backend: super::super::core::backend::CacheBackend,
        key: Cow<'static, str>,
    ) -> Self {
        let pool = match backend {
            super::super::core::backend::CacheBackend::Redis(pool) => pool,
            _ => panic!("RedisCache can only be created from Redis backend"),
        };

        Self {
            pool,
            key,
            __phantom: PhantomData,
        }
    }
}

/// Implement the backend-agnostic CacheTrait for Redis cache operations
#[async_trait]
impl<T> CacheTrait for RedisCache<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Clone + Send + Sync + 'static,
{
    type Value = T;

    async fn exists(&mut self, key: &str) -> CacheResult<bool> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| CacheError::Other(e.to_string()))?;
        conn.exists(key)
            .await
            .map_err(|e| CacheError::Other(e.to_string()))
    }

    async fn get(&mut self, key: &str) -> CacheResult<Self::Value> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| CacheError::Other(e.to_string()))?;
        let json: Json<T> = conn.get(key).await.map_err(|e| {
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
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| CacheError::Other(e.to_string()))?;
        let _: () = conn
            .set(key, json)
            .await
            .map_err(|e| CacheError::Other(e.to_string()))?;
        Ok(())
    }

    async fn set_with_ttl(
        &mut self, key: &str, value: &Self::Value, ttl: Duration,
    ) -> CacheResult<()> {
        let json = Json(value.clone());
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| CacheError::Other(e.to_string()))?;
        let _: () = conn
            .set_ex(key, json, ttl.as_secs() as _)
            .await
            .map_err(|e| CacheError::Other(e.to_string()))?;
        Ok(())
    }

    async fn set_if_not_exist(
        &mut self, key: &str, value: &Self::Value,
    ) -> CacheResult<bool> {
        let json = Json(value.clone());
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| CacheError::Other(e.to_string()))?;
        let result: bool = conn
            .set_nx(key, json)
            .await
            .map_err(|e| CacheError::Other(e.to_string()))?;
        Ok(result)
    }

    async fn remove(&mut self, key: &str) -> CacheResult<bool> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| CacheError::Other(e.to_string()))?;
        let count: u32 = conn
            .del(key)
            .await
            .map_err(|e| CacheError::Other(e.to_string()))?;
        Ok(count > 0)
    }
}
