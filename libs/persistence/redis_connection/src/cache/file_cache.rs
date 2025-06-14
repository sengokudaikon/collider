#![allow(unused)]
//! File-based caching layer implementation
//!
//! This module provides persistent file-based caching as a third layer in the
//! caching system. Useful for data that should survive server restarts.

use std::{
    borrow::Cow, marker::PhantomData, path::PathBuf,
    sync::Arc,
};

use bytes::Bytes;
use deadpool_redis::redis::{
    FromRedisValue, RedisResult, ToRedisArgs, Value,
};
use moka::future::Cache;
use redis::AsyncCommands;
#[cfg(feature = "file-cache")] use sled::Db;
#[cfg(feature = "file-cache")] use tokio::sync::RwLock;

#[cfg(feature = "file-cache")]
use crate::config::FileConfig;
use crate::core::{value::{Json, CacheValue}, type_bind::RedisTypeTrait};
use serde::{Serialize, Deserialize};

/// File-based cache implementation using sled database
#[cfg(feature = "file-cache")]
pub struct FileCache<'redis, R, T> {
    redis: &'redis mut R,
    key: Cow<'static, str>,
    file_db: Arc<RwLock<Db>>,
    tree_name: String,
    config: FileConfig,
    __phantom: PhantomData<T>,
}

#[cfg(feature = "file-cache")]
impl<'redis, R, T> RedisTypeTrait<'redis, R> for FileCache<'redis, R, T> {
    fn from_redis_and_key(
        redis: &'redis mut R, key: Cow<'static, str>,
        _memory: Option<Cache<String, Bytes>>,
    ) -> Self {
        let config = FileConfig {
            path: PathBuf::from("/tmp/redis_file_cache"),
            max_size_mb: 1024,
        };

        let file_db = Arc::new(RwLock::new(
            sled::open(&config.path)
                .expect("Failed to open file cache database"),
        ));

        Self {
            redis,
            key: key.clone(),
            file_db,
            tree_name: format!("cache_{}", key.replace(':', "_")),
            config,
            __phantom: PhantomData,
        }
    }
}

#[cfg(feature = "file-cache")]
impl<'redis, R, T> FileCache<'redis, R, T>
where
    R: deadpool_redis::redis::aio::ConnectionLike + Send + Sync,
    T: Serialize + for<'de> Deserialize<'de> + Clone + Send + Sync + 'redis,
{
    pub fn with_config(mut self, config: FileConfig) -> Self {
        self.config = config;
        self.file_db = Arc::new(RwLock::new(
            sled::open(&self.config.path)
                .expect("Failed to open file cache database"),
        ));
        self
    }

    /// Check if key exists in any layer (file -> Redis)
    pub async fn exists<RV>(&mut self) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        // Check file cache first
        if self.exists_in_file().await.unwrap_or(false) {
            return FromRedisValue::from_redis_value(&Value::Int(1));
        }

        // Check Redis
        self.redis.exists(&*self.key).await
    }

    /// Get value from layered cache (file -> Redis)
    pub async fn get(&mut self) -> RedisResult<T> {
        // Try file cache first
        if let Some(value) = self.get_from_file().await.unwrap_or(None) {
            return Ok(value);
        }

        // Get from Redis
        let json: Json<T> = self.redis.get(&*self.key).await?;
        Ok(json.inner())
    }

    /// Try to get value, returning None if not found
    pub async fn try_get(&mut self) -> RedisResult<Option<T>> {
        if !bool::from_redis_value(&self.exists::<Value>().await?)? {
            return Ok(None);
        }
        self.get().await.map(Some)
    }

    /// Set value in all layers
    pub async fn set<RV>(&mut self, value: impl Into<Json<T>> + Clone) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        let json = value.clone().into();
        // Set in Redis first
        let result: RV = self.redis.set(&*self.key, json.clone()).await?;

        // Cache in file for persistence
        let _ = self.set_in_file(&json.inner()).await;

        Ok(result)
    }

    /// Set value with expiration
    pub async fn set_with_expire<RV>(
        &mut self, value: impl Into<Json<T>> + Clone, duration: std::time::Duration,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        let json = value.clone().into();
        // Set in Redis with TTL
        let result: RV = self
            .redis
            .set_ex(&*self.key, json.clone(), duration.as_secs() as _)
            .await?;

        // Set in file cache with expiration metadata
        let _ = self.set_in_file_with_ttl(&json.inner(), duration).await;

        Ok(result)
    }

    /// Set value only if it doesn't exist
    pub async fn set_if_not_exist<RV>(
        &mut self, value: impl Into<Json<T>> + Clone,
    ) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        // Check if exists in any layer
        if bool::from_redis_value(&self.exists::<Value>().await?)? {
            return FromRedisValue::from_redis_value(&Value::Int(0));
        }

        let json = value.clone().into();
        // Set in Redis
        let result = self.redis.set_nx(&*self.key, json.clone()).await;

        if let Ok(_val) = &result {
            // Cache in file
            let _ = self.set_in_file(&json.inner()).await;
        }

        result
    }

    /// Remove value from all layers
    pub async fn remove<RV>(&mut self) -> RedisResult<RV>
    where
        RV: FromRedisValue,
    {
        // Remove from file cache
        let _ = self.remove_from_file().await;

        // Remove from Redis
        self.redis.del(&*self.key).await
    }

    /// File cache operations
    async fn exists_in_file(&self) -> Result<bool, sled::Error> {
        let db = self.file_db.read().await;
        let tree = db.open_tree(&self.tree_name)?;
        tree.contains_key(&*self.key)
    }

    async fn get_from_file(
        &self,
    ) -> Result<Option<T>, Box<dyn std::error::Error + Send + Sync>>
    {
        let db = self.file_db.read().await;
        let tree = db.open_tree(&self.tree_name)?;

        if let Some(data) = tree.get(&*self.key)? {
            // Check if expired
            if self.is_expired(&data).await? {
                drop(db);
                let _ = self.remove_from_file().await;
                return Ok(None);
            }

            let payload = self.extract_payload(&data)?;
            let json = Json::<T>::from_bytes(&payload)
                .map_err(|e| format!("Deserialization failed: {}", e))?;
            Ok(Some(json.inner()))
        }
        else {
            Ok(None)
        }
    }

    async fn set_in_file(
        &self, value: &T,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
    {
        let json = Json(value.clone());
        let bytes = json.to_bytes()
            .map_err(|e| format!("Serialization failed: {}", e))?;

        let db = self.file_db.write().await;
        let tree = db.open_tree(&self.tree_name)?;
        tree.insert(&*self.key, bytes)?;
        tree.flush_async().await?;
        Ok(())
    }

    async fn set_in_file_raw(
        &self, value: &T,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
    {
        let json = Json(value.clone());
        let bytes = json.to_bytes()
            .map_err(|e| format!("Serialization failed: {}", e))?;

        let db = self.file_db.write().await;
        let tree = db.open_tree(&self.tree_name)?;
        tree.insert(&*self.key, bytes)?;
        tree.flush_async().await?;
        Ok(())
    }

    async fn set_in_file_with_ttl(
        &self, value: &T, ttl: std::time::Duration,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
    {
        let json = Json(value.clone());
        let bytes = json.to_bytes()
            .map_err(|e| format!("Serialization failed: {}", e))?;

        // Prepend expiration timestamp
        let expiry = std::time::SystemTime::now() + ttl;
        let expiry_bytes = expiry
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs()
            .to_be_bytes();

        let mut payload = expiry_bytes.to_vec();
        payload.extend(bytes);

        let db = self.file_db.write().await;
        let tree = db.open_tree(&self.tree_name)?;
        tree.insert(&*self.key, payload)?;
        tree.flush_async().await?;
        Ok(())
    }

    async fn remove_from_file(&self) -> Result<(), sled::Error> {
        let db = self.file_db.write().await;
        let tree = db.open_tree(&self.tree_name)?;
        tree.remove(&*self.key)?;
        tree.flush_async().await?;
        Ok(())
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
#[derive(Debug)]
pub struct FileCacheStats {
    pub total_keys: usize,
    pub size_on_disk: u64,
    pub max_size_bytes: u64,
}

// Stubs when file-cache feature is disabled
#[cfg(not(feature = "file-cache"))]
pub struct FileCache<'redis, R, T> {
    _phantom: PhantomData<(&'redis R, T)>,
}

#[cfg(not(feature = "file-cache"))]
impl<'redis, R, T> RedisTypeTrait<'redis, R> for FileCache<'redis, R, T> {
    fn from_redis_and_key(
        _redis: &'redis mut R, _key: Cow<'static, str>,
        _memory: Option<Cache<String, Bytes>>,
    ) -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}
