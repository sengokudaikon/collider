use std::{borrow::Cow, marker::PhantomData, time::Duration};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument, warn};

use super::{
    memory::Memory,
    redis_cache::RedisCache,
    r#trait::{CacheError, CacheResult, CacheTrait},
};
#[cfg(feature = "file-cache")]
use super::file_cache::FileCache;
use crate::{
    config::TieredConfig,
    core::{
        backend::CacheBackend,
        type_bind::CacheTypeTrait,
    },
};

/// Tiered cache that manages multiple cache instances in order of speed
/// Implements read-through, write-through patterns with configurable policies
pub struct Tiered<'cache, T> {
    caches: Vec<Box<dyn CacheTrait<Value = T> + Send + Sync + 'cache>>,
    config: TieredConfig,
    __phantom: PhantomData<(&'cache (), T)>,
}

impl<'cache, T> CacheTypeTrait<'cache> for Tiered<'cache, T>
where
    T: Serialize + for<'de> Deserialize<'de> + Clone + Send + Sync + 'static,
{
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

        // Convert backends to cache instances
        let mut caches: Vec<Box<dyn CacheTrait<Value = T> + Send + Sync + 'cache>> = Vec::new();
        
        for backend in backends.iter() {
            match backend {
                CacheBackend::Memory { cache, config } => {
                    let memory_cache = Memory::<T>::from_cache_and_key(
                        CacheBackend::Memory {
                            cache: cache.clone(),
                            config: config.clone(),
                        },
                        key.clone(),
                    );
                    caches.push(Box::new(memory_cache));
                }
                CacheBackend::Redis(pool) => {
                    let redis_cache = RedisCache::<T>::from_cache_and_key(
                        CacheBackend::Redis(pool.clone()),
                        key.clone(),
                    );
                    caches.push(Box::new(redis_cache));
                }
                #[cfg(feature = "file-cache")]
                CacheBackend::File { file_db, config } => {
                    let file_cache = FileCache::<T>::from_cache_and_key(
                        CacheBackend::File {
                            file_db: file_db.clone(),
                            config: config.clone(),
                        },
                        key.clone(),
                    );
                    caches.push(Box::new(file_cache));
                }
                CacheBackend::Tiered { .. } => {
                    panic!("Nested tiered caches not supported");
                }
            }
        }

        Self {
            caches,
            config,
            __phantom: PhantomData,
        }
    }
}

impl<'cache, T> Tiered<'cache, T>
where
    T: Serialize + for<'de> Deserialize<'de> + Clone + Send + Sync + 'static,
{
    pub fn with_config(mut self, config: TieredConfig) -> Self {
        self.config = config;
        self
    }
}

/// Implement the backend-agnostic CacheTrait for tiered cache operations
#[async_trait]
impl<'cache, T> CacheTrait for Tiered<'cache, T>
where
    T: Serialize + for<'de> Deserialize<'de> + Clone + Send + Sync + 'static,
{
    type Value = T;

    async fn exists(&mut self, key: &str) -> CacheResult<bool> {
        // Check each cache in order (fastest to slowest)
        for cache in &mut self.caches {
            if cache.exists(key).await? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn get(&mut self, key: &str) -> CacheResult<Self::Value> {
        // Try each cache in order until we find the value
        for (layer_idx, cache) in self.caches.iter_mut().enumerate() {
            if let Some(value) = cache.try_get(key).await? {
                // Populate faster caches if enabled and we found the value in a slower layer
                if self.config.populate_on_read && layer_idx > 0 {
                    debug!("Populating faster cache layers with value from layer {}", layer_idx);
                    self.populate_faster_layers(key, &value, layer_idx).await?;
                }
                return Ok(value);
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
        match self.config.write_strategy {
            crate::config::WriteStrategy::WriteThrough => {
                // Write to all caches
                for cache in &mut self.caches {
                    cache.set(key, value).await?;
                }
                Ok(())
            }
            crate::config::WriteStrategy::WriteBack => {
                // Write to fastest cache only
                if let Some(cache) = self.caches.first_mut() {
                    cache.set(key, value).await
                } else {
                    Err(CacheError::Other("No caches available".to_string()))
                }
            }
            crate::config::WriteStrategy::WriteToSlowest => {
                // Write to most persistent cache only
                if let Some(cache) = self.caches.last_mut() {
                    cache.set(key, value).await
                } else {
                    Err(CacheError::Other("No caches available".to_string()))
                }
            }
        }
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

        // Remove from all caches
        for cache in &mut self.caches {
            if cache.remove(key).await? {
                existed = true;
            }
        }

        Ok(existed)
    }
}

impl<'cache, T> Tiered<'cache, T>
where
    T: Serialize + for<'de> Deserialize<'de> + Clone + Send + Sync + 'static,
{
    /// Helper method to populate faster cache layers with a value
    async fn populate_faster_layers(&mut self, key: &str, value: &T, found_at_layer: usize) -> CacheResult<()> {
        // Populate all layers before the one where we found the value
        for (layer_idx, cache) in self.caches.iter_mut().enumerate() {
            if layer_idx >= found_at_layer {
                break; // Don't populate the layer where we found it or slower ones
            }
            let _ = cache.set(key, value).await;
        }
        Ok(())
    }
}
