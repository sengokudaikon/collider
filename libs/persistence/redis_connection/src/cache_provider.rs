use std::sync::{Arc, OnceLock};

use crate::core::backend::CacheBackend;

// Store Arc<CacheBackend> for efficient cloning
static CACHE_BACKEND: OnceLock<Arc<CacheBackend<'static>>> = OnceLock::new();

pub struct CacheProvider;

impl CacheProvider {
    /// Initialize the global cache backend with a Redis pool (common case)
    pub fn init_redis_static(pool: deadpool_redis::Pool) {
        let backend = Arc::new(CacheBackend::Redis(pool));
        CACHE_BACKEND.set(backend).ok();
    }

    /// Initialize the global cache backend with a memory cache (for testing)
    pub fn init_memory_static(config: crate::config::MemoryConfig) {
        let cache = moka::future::Cache::builder()
            .max_capacity(config.capacity)
            .time_to_live(config.ttl())
            .build();
        let backend = Arc::new(CacheBackend::Memory { cache, config });
        CACHE_BACKEND.set(backend).ok();
    }

    /// Get a clone of the global cache backend (cheap Arc clone)
    pub fn get_backend() -> Arc<CacheBackend<'static>> {
        CACHE_BACKEND
            .get()
            .expect(
                "Cache backend not initialized. Call \
                 CacheProvider::init_*_static() first",
            )
            .clone()
    }

    /// Create a Redis-based cache backend from a pool
    pub fn redis_backend(
        pool: deadpool_redis::Pool,
    ) -> CacheBackend<'static> {
        CacheBackend::Redis(pool)
    }

    /// Create a memory-based cache backend
    pub fn memory_backend(
        cache: moka::future::Cache<String, bytes::Bytes>,
        config: crate::config::MemoryConfig,
    ) -> CacheBackend<'static> {
        CacheBackend::Memory { cache, config }
    }

    /// Create a memory-based cache backend with default configuration
    pub fn default_memory_backend() -> CacheBackend<'static> {
        let config = crate::config::MemoryConfig::default();
        let cache = moka::future::Cache::builder()
            .max_capacity(config.capacity)
            .time_to_live(config.ttl())
            .build();
        CacheBackend::Memory { cache, config }
    }

    #[cfg(feature = "file-cache")]
    /// Create a file-based cache backend
    pub fn file_backend(
        file_db: std::sync::Arc<tokio::sync::RwLock<sled::Db>>,
        config: crate::config::FileConfig,
    ) -> CacheBackend<'static> {
        CacheBackend::File { file_db, config }
    }

    #[cfg(feature = "file-cache")]
    /// Create a file-based cache backend from a path
    pub fn file_backend_from_path(
        path: std::path::PathBuf, config: crate::config::FileConfig,
    ) -> Result<CacheBackend<'static>, String> {
        let db = sled::open(&path).map_err(|e| {
            format!("Failed to open file cache at {}: {}", path.display(), e)
        })?;
        let file_db = std::sync::Arc::new(tokio::sync::RwLock::new(db));
        Ok(CacheBackend::File { file_db, config })
    }

    /// Create a tiered cache backend using the builder pattern
    pub fn tiered_builder(
        config: crate::config::TieredConfig,
    ) -> crate::core::backend::TieredCacheBuilder<'static> {
        crate::core::backend::CacheBackend::tiered_builder(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_backend_creation() {
        let backend = CacheProvider::default_memory_backend();
        assert!(!backend.is_redis());
    }
}
