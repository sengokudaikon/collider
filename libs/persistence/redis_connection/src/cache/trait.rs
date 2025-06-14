use std::borrow::Cow;
use std::time::Duration;

/// Cache-specific error type that doesn't depend on Redis
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("Key not found")]
    KeyNotFound,
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Deserialization error: {0}")]
    DeserializationError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Operation not supported: {0}")]
    Unsupported(String),
    
    #[error("Other error: {0}")]
    Other(String),
}

pub type CacheResult<T> = Result<T, CacheError>;

/// A cache trait that doesn't require Redis connection or lifetimes.
/// This allows Memory, FileCache, and other cache implementations to exist independently.
#[async_trait::async_trait]
pub trait CacheTrait: Send + Sync {
    type Value: serde::Serialize + serde::de::DeserializeOwned + Clone + Send + Sync;
    
    /// Check if key exists in cache
    async fn exists(&self, key: &str) -> CacheResult<bool>;
    
    /// Get value from cache
    async fn get(&self, key: &str) -> CacheResult<Self::Value>;
    
    /// Get value from cache, returning None if not found
    async fn try_get(&self, key: &str) -> CacheResult<Option<Self::Value>>;
    
    /// Set value in cache
    async fn set(&self, key: &str, value: &Self::Value) -> CacheResult<()>;
    
    /// Set value with expiration
    async fn set_with_ttl(&self, key: &str, value: &Self::Value, ttl: Duration) -> CacheResult<()>;
    
    /// Set value only if it doesn't exist
    async fn set_if_not_exist(&self, key: &str, value: &Self::Value) -> CacheResult<bool>;
    
    /// Remove key from cache
    async fn remove(&self, key: &str) -> CacheResult<bool>;
    
    /// Clear all entries (optional operation)
    async fn clear(&self) -> CacheResult<()> {
        Err(CacheError::Unsupported(
            "Clear operation not supported by this cache implementation".to_string()
        ))
    }
}

/// Extension trait for cache implementations that support key patterns
#[async_trait::async_trait]
pub trait CachePatternTrait: CacheTrait {
    /// Remove all keys matching a pattern
    async fn remove_pattern(&self, pattern: &str) -> CacheResult<u64>;
    
    /// Get all keys matching a pattern
    async fn keys(&self, pattern: &str) -> CacheResult<Vec<String>>;
}

/// A cache key provider that generates cache keys
pub trait CacheKeyProvider {
    type Args<'a>;
    
    fn get_key(&self, args: Self::Args<'_>) -> Cow<'static, str>;
}