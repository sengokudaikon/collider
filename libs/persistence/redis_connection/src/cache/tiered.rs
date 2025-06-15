use std::{borrow::Cow, marker::PhantomData, time::Duration};

use async_trait::async_trait;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::r#trait::{CacheError, CacheResult, CacheTrait};
use crate::{
    config::TieredConfig,
    core::{
        backend::CacheBackend,
        type_bind::CacheTypeTrait,
        value::{CacheValue, Json},
    },
};

pub struct Tiered<'cache, T> {
    backends: super::super::core::backend::BoundedBackends<'cache>,
    #[allow(dead_code)]
    key: Cow<'static, str>,
    config: TieredConfig,
    __phantom: PhantomData<(&'cache (), T)>,
}

impl<'cache, T> CacheTypeTrait<'cache> for Tiered<'cache, T> {
    #[instrument(skip(backend), fields(key = %key))]
    fn from_cache_and_key(
        backend: CacheBackend<'cache>, key: Cow<'static, str>,
    ) -> Self {
        let (backends, config) = match backend {
            CacheBackend::Tiered { backends, config } => (backends, config),
            _ => {
                panic!("Tiered cache can only be created from Tiered backend")
            }
        };

        Self {
            backends,
            key,
            config,
            __phantom: PhantomData,
        }
    }
}

impl<'cache, T> Tiered<'cache, T>
where
    T: Serialize + for<'de> Deserialize<'de> + Clone + Send + Sync + 'cache,
{
    pub fn with_config(mut self, config: TieredConfig) -> Self {
        self.config = config;
        self
    }
}

/// Implement the backend-agnostic CacheTrait for tiered cache operations
#[async_trait]
impl<T> CacheTrait for Tiered<'_, T>
where
    T: Serialize + for<'de> Deserialize<'de> + Clone + Send + Sync + 'static,
{
    type Value = T;

    async fn exists(&mut self, key: &str) -> CacheResult<bool> {
        // Check each backend in order (fastest to slowest)
        for backend in self.backends.iter() {
            match backend {
                CacheBackend::Memory { cache, .. } => {
                    if cache.get(key).await.is_some() {
                        return Ok(true);
                    }
                }
                CacheBackend::Redis(_redis) => {
                    // Note: We can't actually use Redis here without
                    // AsyncCommands trait This would need
                    // to be implemented with proper Redis trait bounds
                    // For now, we'll assume it doesn't exist at Redis level
                    return Ok(false);
                }
                #[cfg(feature = "file-cache")]
                CacheBackend::File { .. } => {
                    // File cache existence check would go here
                    // For now, assume not found
                }
                _ => {}
            }
        }
        Ok(false)
    }

    async fn get(&mut self, key: &str) -> CacheResult<Self::Value> {
        // Try each backend in order until we find the value
        for backend in self.backends.iter() {
            match backend {
                CacheBackend::Memory { cache, .. } => {
                    if let Some(bytes) = cache.get(key).await {
                        let json =
                            Json::<T>::from_bytes(&bytes).map_err(|e| {
                                CacheError::DeserializationError(
                                    e.to_string(),
                                )
                            })?;

                        // If populate_on_read is enabled, populate faster
                        // caches
                        if self.config.populate_on_read {
                            // Would populate faster backends here
                        }

                        return Ok(json.inner());
                    }
                }
                CacheBackend::Redis(_redis) => {
                    // Redis lookup would go here with proper trait bounds
                    // For now, continue to next backend
                }
                #[cfg(feature = "file-cache")]
                CacheBackend::File { .. } => {
                    // File cache lookup would go here
                }
                _ => {}
            }
        }
        Err(CacheError::KeyNotFound)
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
        let bytes = json
            .to_bytes()
            .map_err(|e| CacheError::SerializationError(e.to_string()))?;

        // Set in all backends based on write strategy
        for backend in self.backends.iter() {
            match backend {
                CacheBackend::Memory { cache, .. } => {
                    cache
                        .insert(key.to_string(), Bytes::from(bytes.clone()))
                        .await;
                }
                CacheBackend::Redis(_redis) => {
                    // Redis set would go here with proper trait bounds
                }
                #[cfg(feature = "file-cache")]
                CacheBackend::File { .. } => {
                    // File cache set would go here
                }
                _ => {}
            }
        }
        Ok(())
    }

    async fn set_with_ttl(
        &mut self, key: &str, value: &Self::Value, _ttl: Duration,
    ) -> CacheResult<()> {
        // Similar to set but with TTL support where available
        self.set(key, value).await
    }

    async fn set_if_not_exist(
        &mut self, key: &str, value: &Self::Value,
    ) -> CacheResult<bool> {
        if self.exists(key).await? {
            Ok(false)
        }
        else {
            self.set(key, value).await?;
            Ok(true)
        }
    }

    async fn remove(&mut self, key: &str) -> CacheResult<bool> {
        let mut existed = false;

        // Remove from all backends
        for backend in self.backends.iter() {
            match backend {
                CacheBackend::Memory { cache, .. } => {
                    if cache.get(key).await.is_some() {
                        existed = true;
                    }
                    cache.invalidate(key).await;
                }
                CacheBackend::Redis(_redis) => {
                    // Redis remove would go here
                }
                #[cfg(feature = "file-cache")]
                CacheBackend::File { .. } => {
                    // File cache remove would go here
                }
                _ => {}
            }
        }

        Ok(existed)
    }
}
