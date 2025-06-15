#![allow(unused)]
//! File-based caching layer implementation
//!
//! This module provides persistent file-based caching as a third layer in the
//! caching system. Useful for data that should survive server restarts.

use std::{borrow::Cow, marker::PhantomData, sync::Arc, time::Duration};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
#[cfg(feature = "file-cache")] use sled::Db;
#[cfg(feature = "file-cache")] use tokio::sync::RwLock;

#[cfg(feature = "file-cache")]
use crate::config::FileConfig;
use crate::{
    cache::r#trait::{CacheError, CacheResult, CacheTrait},
    core::{
        type_bind::CacheTypeTrait,
        value::{CacheValue, Json},
    },
};

/// File-based cache implementation using sled database
#[cfg(feature = "file-cache")]
pub struct FileCache<'cache, T> {
    key: Cow<'static, str>,
    file_db: Arc<RwLock<Db>>,
    tree_name: String,
    config: FileConfig,
    __phantom: PhantomData<(&'cache (), T)>,
}

#[cfg(feature = "file-cache")]
impl<'cache, T> CacheTypeTrait<'cache> for FileCache<'cache, T> {
    fn from_cache_and_key(
        backend: super::super::core::backend::CacheBackend<'cache>,
        key: Cow<'static, str>,
    ) -> Self {
        let (file_db, config) = match backend {
            super::super::core::backend::CacheBackend::File {
                file_db,
                config,
            } => (file_db, config),
            _ => panic!("FileCache can only be created from File backend"),
        };

        Self {
            key: key.clone(),
            file_db,
            tree_name: format!("cache_{}", key.replace(':', "_")),
            config,
            __phantom: PhantomData,
        }
    }
}

#[cfg(feature = "file-cache")]
impl<'cache, T> FileCache<'cache, T>
where
    T: Serialize + for<'de> Deserialize<'de> + Clone + Send + Sync + 'cache,
{
    pub fn with_config(mut self, config: FileConfig) -> Self {
        self.config = config;
        // Note: file_db is now provided via the backend, not opened here
        self
    }

    async fn is_expired(
        &self, data: &[u8],
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        if data.len() < 8 {
            return Ok(false); // No expiry data
        }

        let expiry_bytes: [u8; 8] = data[0..8].try_into()?;
        let expiry_secs = u64::from_be_bytes(expiry_bytes);
        let expiry = std::time::UNIX_EPOCH
            + std::time::Duration::from_secs(expiry_secs);

        Ok(std::time::SystemTime::now() > expiry)
    }

    fn extract_payload(
        &self, data: &[u8],
    ) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        if data.len() >= 8 {
            // Has expiry timestamp, extract payload
            Ok(data[8..].to_vec())
        }
        else {
            // No expiry, entire data is payload
            Ok(data.to_vec())
        }
    }

    /// Clean up expired entries
    pub async fn cleanup_expired(&self) -> Result<usize, sled::Error> {
        let db = self.file_db.write().await;
        let tree = db.open_tree(&self.tree_name)?;
        let mut removed_count = 0;

        for item in tree.iter() {
            let (key, data) = item?;
            if self.is_expired(&data).await.unwrap_or(false) {
                tree.remove(key)?;
                removed_count += 1;
            }
        }

        tree.flush_async().await?;
        Ok(removed_count)
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> Result<FileCacheStats, sled::Error> {
        let db = self.file_db.read().await;
        let tree = db.open_tree(&self.tree_name)?;

        let total_keys = tree.len();
        let size_on_disk = db.size_on_disk()?;

        Ok(FileCacheStats {
            total_keys,
            size_on_disk,
            max_size_bytes: self.config.max_size_mb * 1024 * 1024,
        })
    }
}

#[cfg(feature = "file-cache")]
#[async_trait]
impl<T> CacheTrait for FileCache<'_, T>
where
    T: Serialize + for<'de> Deserialize<'de> + Clone + Send + Sync + 'static,
{
    type Value = T;

    async fn exists(&mut self, key: &str) -> CacheResult<bool> {
        let db = self.file_db.read().await;
        let tree = db
            .open_tree(&self.tree_name)
            .map_err(|e| CacheError::Other(e.to_string()))?;
        tree.contains_key(key)
            .map_err(|e| CacheError::Other(e.to_string()))
    }

    async fn get(&mut self, key: &str) -> CacheResult<Self::Value> {
        let db = self.file_db.read().await;
        let tree = db
            .open_tree(&self.tree_name)
            .map_err(|e| CacheError::Other(e.to_string()))?;

        if let Some(data) = tree
            .get(key)
            .map_err(|e| CacheError::Other(e.to_string()))?
        {
            // Check if expired
            if self
                .is_expired(&data)
                .await
                .map_err(|e| CacheError::Other(e.to_string()))?
            {
                drop(db);
                // Remove expired key
                let db = self.file_db.write().await;
                let tree = db
                    .open_tree(&self.tree_name)
                    .map_err(|e| CacheError::Other(e.to_string()))?;
                let _ = tree.remove(key);
                let _ = tree.flush_async().await;
                return Err(CacheError::KeyNotFound);
            }

            let payload = self
                .extract_payload(&data)
                .map_err(|e| CacheError::Other(e.to_string()))?;
            let json = Json::<T>::from_bytes(&payload).map_err(|e| {
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

        let db = self.file_db.write().await;
        let tree = db
            .open_tree(&self.tree_name)
            .map_err(|e| CacheError::Other(e.to_string()))?;
        tree.insert(key, bytes)
            .map_err(|e| CacheError::Other(e.to_string()))?;
        tree.flush_async()
            .await
            .map_err(|e| CacheError::Other(e.to_string()))?;
        Ok(())
    }

    async fn set_with_ttl(
        &mut self, key: &str, value: &Self::Value, ttl: Duration,
    ) -> CacheResult<()> {
        let json = Json(value.clone());
        let bytes = json
            .to_bytes()
            .map_err(|e| CacheError::SerializationError(e.to_string()))?;

        // Prepend expiration timestamp
        let expiry = std::time::SystemTime::now() + ttl;
        let expiry_bytes = expiry
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| CacheError::Other(e.to_string()))?
            .as_secs()
            .to_be_bytes();

        let mut payload = expiry_bytes.to_vec();
        payload.extend(bytes);

        let db = self.file_db.write().await;
        let tree = db
            .open_tree(&self.tree_name)
            .map_err(|e| CacheError::Other(e.to_string()))?;
        tree.insert(key, payload)
            .map_err(|e| CacheError::Other(e.to_string()))?;
        tree.flush_async()
            .await
            .map_err(|e| CacheError::Other(e.to_string()))?;
        Ok(())
    }

    async fn set_if_not_exist(
        &mut self, key: &str, value: &Self::Value,
    ) -> CacheResult<bool> {
        if self.exists(key).await? {
            Ok(false) // Already exists
        }
        else {
            self.set(key, value).await?;
            Ok(true) // Successfully set
        }
    }

    async fn remove(&mut self, key: &str) -> CacheResult<bool> {
        let db = self.file_db.write().await;
        let tree = db
            .open_tree(&self.tree_name)
            .map_err(|e| CacheError::Other(e.to_string()))?;
        let existed = tree
            .remove(key)
            .map_err(|e| CacheError::Other(e.to_string()))?
            .is_some();
        tree.flush_async()
            .await
            .map_err(|e| CacheError::Other(e.to_string()))?;
        Ok(existed)
    }
}

#[cfg(feature = "file-cache")]
#[derive(Debug)]
pub struct FileCacheStats {
    pub total_keys: usize,
    pub size_on_disk: u64,
    pub max_size_bytes: u64,
}

// Stubs when file-cache feature is disabled
#[cfg(not(feature = "file-cache"))]
pub struct FileCache<'cache, T> {
    _phantom: PhantomData<(&'cache (), T)>,
}

#[cfg(not(feature = "file-cache"))]
impl<'cache, T> CacheTypeTrait<'cache> for FileCache<'cache, T> {
    fn from_cache_and_key(
        _backend: super::super::core::backend::CacheBackend<'cache>,
        _key: Cow<'static, str>,
    ) -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

#[cfg(not(feature = "file-cache"))]
#[async_trait]
impl<T> CacheTrait for FileCache<'_, T>
where
    T: Serialize + for<'de> Deserialize<'de> + Clone + Send + Sync + 'static,
{
    type Value = T;

    async fn exists(&self, _key: &str) -> CacheResult<bool> {
        Err(CacheError::Unsupported(
            "File cache not compiled".to_string(),
        ))
    }

    async fn get(&self, _key: &str) -> CacheResult<Self::Value> {
        Err(CacheError::Unsupported(
            "File cache not compiled".to_string(),
        ))
    }

    async fn try_get(&self, _key: &str) -> CacheResult<Option<Self::Value>> {
        Err(CacheError::Unsupported(
            "File cache not compiled".to_string(),
        ))
    }

    async fn set(&self, _key: &str, _value: &Self::Value) -> CacheResult<()> {
        Err(CacheError::Unsupported(
            "File cache not compiled".to_string(),
        ))
    }

    async fn set_with_ttl(
        &self, _key: &str, _value: &Self::Value, _ttl: Duration,
    ) -> CacheResult<()> {
        Err(CacheError::Unsupported(
            "File cache not compiled".to_string(),
        ))
    }

    async fn set_if_not_exist(
        &self, _key: &str, _value: &Self::Value,
    ) -> CacheResult<bool> {
        Err(CacheError::Unsupported(
            "File cache not compiled".to_string(),
        ))
    }

    async fn remove(&self, _key: &str) -> CacheResult<bool> {
        Err(CacheError::Unsupported(
            "File cache not compiled".to_string(),
        ))
    }
}
