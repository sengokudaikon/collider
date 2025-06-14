use std::marker::PhantomData;
use std::time::Duration;

use bytes::Bytes;
use moka::future::Cache;

use crate::config::MemoryConfig;
use crate::core::value::{Json, CacheValue};
use super::r#trait::{CacheTrait, CachePatternTrait, CacheResult, CacheError};

/// A standalone memory cache that doesn't depend on Redis lifetimes
pub struct MemoryCache<T> {
    cache: Cache<String, Bytes>,
    config: MemoryConfig,
    _phantom: PhantomData<T>,
}

impl<T> MemoryCache<T> {
    pub fn new(config: MemoryConfig) -> Self {
        let cache = Cache::builder()
            .max_capacity(config.capacity)
            .time_to_live(config.ttl())
            .build();
        
        Self {
            cache,
            config,
            _phantom: PhantomData,
        }
    }
    
    pub fn with_cache(cache: Cache<String, Bytes>, config: MemoryConfig) -> Self {
        Self {
            cache,
            config,
            _phantom: PhantomData,
        }
    }
}

#[async_trait::async_trait]
impl<T> CacheTrait for MemoryCache<T>
where
    T: serde::Serialize + serde::de::DeserializeOwned + Clone + Send + Sync + 'static,
{
    type Value = T;
    
    async fn exists(&self, key: &str) -> CacheResult<bool> {
        Ok(self.cache.get(key).await.is_some())
    }
    
    async fn get(&self, key: &str) -> CacheResult<Self::Value> {
        if let Some(bytes) = self.cache.get(key).await {
            let json = Json::<T>::from_bytes(&bytes)
                .map_err(|e| CacheError::DeserializationError(e.to_string()))?;
            Ok(json.inner())
        } else {
            Err(CacheError::KeyNotFound)
        }
    }
    
    async fn try_get(&self, key: &str) -> CacheResult<Option<Self::Value>> {
        if let Some(bytes) = self.cache.get(key).await {
            let json = Json::<T>::from_bytes(&bytes)
                .map_err(|e| CacheError::DeserializationError(e.to_string()))?;
            Ok(Some(json.inner()))
        } else {
            Ok(None)
        }
    }
    
    async fn set(&self, key: &str, value: &Self::Value) -> CacheResult<()> {
        let json = Json(value.clone());
        let bytes = json.to_bytes()
            .map_err(|e| CacheError::SerializationError(e.to_string()))?;
        self.cache.insert(key.to_string(), Bytes::from(bytes)).await;
        Ok(())
    }
    
    async fn set_with_ttl(&self, key: &str, value: &Self::Value, _ttl: Duration) -> CacheResult<()> {
        // Note: Moka cache uses global TTL, not per-key TTL
        // For per-key TTL, we'd need a different implementation
        self.set(key, value).await
    }
    
    async fn set_if_not_exist(&self, key: &str, value: &Self::Value) -> CacheResult<bool> {
        if self.cache.get(key).await.is_none() {
            self.set(key, value).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    
    async fn remove(&self, key: &str) -> CacheResult<bool> {
        let existed = self.cache.get(key).await.is_some();
        self.cache.invalidate(key).await;
        Ok(existed)
    }
    
    async fn clear(&self) -> CacheResult<()> {
        self.cache.invalidate_all();
        Ok(())
    }
}

#[async_trait::async_trait]
impl<T> CachePatternTrait for MemoryCache<T>
where
    T: serde::Serialize + serde::de::DeserializeOwned + Clone + Send + Sync + 'static,
{
    async fn remove_pattern(&self, _pattern: &str) -> CacheResult<u64> {
        // Note: Pattern removal is not efficiently supported by Moka cache
        // We can't iterate over keys to match patterns without significant overhead
        Err(CacheError::Unsupported(
            "Pattern removal not efficiently supported by Moka cache".to_string()
        ))
    }
    
    async fn keys(&self, _pattern: &str) -> CacheResult<Vec<String>> {
        // Similar limitation as remove_pattern
        Err(CacheError::Unsupported(
            "Pattern key listing not supported by Moka cache".to_string()
        ))
    }
}