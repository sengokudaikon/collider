use std::{borrow::Cow, marker::PhantomData, time::Duration};

use async_trait::async_trait;
use bytes::Bytes;
use moka::future::Cache;
use serde::{Deserialize, Serialize};

use super::r#trait::{CacheError, CacheResult, CacheTrait};
use crate::{
    config::MemoryConfig,
    core::{
        type_bind::CacheTypeTrait,
        value::{CacheValue, Json},
    },
};

pub struct Memory<'cache, T> {
    memory: Cache<String, Bytes>,
    #[allow(dead_code)]
    key: Cow<'static, str>,
    config: MemoryConfig,
    __phantom: PhantomData<(&'cache (), T)>,
}

impl<'cache, T> CacheTypeTrait<'cache> for Memory<'cache, T> {
    fn from_cache_and_key(
        backend: super::super::core::backend::CacheBackend<'cache>,
        key: Cow<'static, str>,
    ) -> Self {
        let (cache, config) = match backend {
            super::super::core::backend::CacheBackend::Memory {
                cache,
                config,
            } => (cache, config),
            _ => {
                panic!("Memory cache can only be created from Memory backend")
            }
        };

        Self {
            memory: cache,
            key,
            config,
            __phantom: PhantomData,
        }
    }
}

impl<'cache, T> Memory<'cache, T>
where
    T: Serialize + for<'de> Deserialize<'de> + Send + Sync + 'cache,
{
    pub fn with_config(mut self, config: MemoryConfig) -> Self {
        self.memory = Cache::builder()
            .max_capacity(config.capacity)
            .time_to_live(config.ttl())
            .build();
        self.config = config;
        self
    }
}

#[async_trait]
impl<T> CacheTrait for Memory<'_, T>
where
    T: Serialize + for<'de> Deserialize<'de> + Clone + Send + Sync + 'static,
{
    type Value = T;

    async fn exists(&mut self, key: &str) -> CacheResult<bool> {
        Ok(self.memory.get(key).await.is_some())
    }

    async fn get(&mut self, key: &str) -> CacheResult<Self::Value> {
        if let Some(bytes) = self.memory.get(key).await {
            let json = Json::<T>::from_bytes(&bytes).map_err(|e| {
                CacheError::DeserializationError(e.to_string())
            })?;
            Ok(json.inner())
        }
        else {
            Err(CacheError::KeyNotFound)
        }
    }

    async fn try_get(
        &mut self, key: &str,
    ) -> CacheResult<Option<Self::Value>> {
        if let Some(bytes) = self.memory.get(key).await {
            let json = Json::<T>::from_bytes(&bytes).map_err(|e| {
                CacheError::DeserializationError(e.to_string())
            })?;
            Ok(Some(json.inner()))
        }
        else {
            Ok(None)
        }
    }

    async fn set(
        &mut self, key: &str, value: &Self::Value,
    ) -> CacheResult<()> {
        let json = Json(value.clone());
        let bytes = json
            .to_bytes()
            .map_err(|e| CacheError::SerializationError(e.to_string()))?;
        self.memory
            .insert(key.to_string(), Bytes::from(bytes))
            .await;
        Ok(())
    }

    async fn set_with_ttl(
        &mut self, key: &str, value: &Self::Value, _ttl: Duration,
    ) -> CacheResult<()> {
        // Note: Moka cache uses global TTL, not per-key TTL
        let json = Json(value.clone());
        let bytes = json
            .to_bytes()
            .map_err(|e| CacheError::SerializationError(e.to_string()))?;
        self.memory
            .insert(key.to_string(), Bytes::from(bytes))
            .await;
        Ok(())
    }

    async fn set_if_not_exist(
        &mut self, key: &str, value: &Self::Value,
    ) -> CacheResult<bool> {
        if self.memory.get(key).await.is_none() {
            let json = Json(value.clone());
            let bytes = json
                .to_bytes()
                .map_err(|e| CacheError::SerializationError(e.to_string()))?;
            self.memory
                .insert(key.to_string(), Bytes::from(bytes))
                .await;
            Ok(true)
        }
        else {
            Ok(false)
        }
    }

    async fn remove(&mut self, key: &str) -> CacheResult<bool> {
        let existed = self.memory.get(key).await.is_some();
        self.memory.invalidate(key).await;
        Ok(existed)
    }

    async fn clear(&mut self) -> CacheResult<()> {
        self.memory.invalidate_all();
        Ok(())
    }
}
