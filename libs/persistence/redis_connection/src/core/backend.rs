use bytes::Bytes;
use moka::future::Cache;

/// A runtime-configurable bounded vector for cache backends
/// This provides the configurability we need while maintaining efficiency
///
/// Unlike fixed-size collections like ArrayVec or SmallVec, this allows
/// the capacity to be determined at runtime from configuration.
pub struct BoundedBackends<'a> {
    backends: Vec<CacheBackend<'a>>,
    max_capacity: usize,
    _phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> BoundedBackends<'a> {
    /// Create a new bounded backends collection with the specified capacity
    pub fn with_capacity(max_capacity: usize) -> Self {
        Self {
            backends: Vec::with_capacity(max_capacity),
            max_capacity,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Add a backend to the collection
    /// Returns an error if the collection is at capacity
    pub fn push(
        &mut self, backend: CacheBackend<'a>,
    ) -> Result<(), CacheBackend<'a>> {
        if self.backends.len() >= self.max_capacity {
            Err(backend)
        }
        else {
            self.backends.push(backend);
            Ok(())
        }
    }

    /// Get the current number of backends
    pub fn len(&self) -> usize { self.backends.len() }

    /// Check if the collection is empty
    pub fn is_empty(&self) -> bool { self.backends.is_empty() }

    /// Get the maximum capacity
    pub fn capacity(&self) -> usize { self.max_capacity }

    /// Get an iterator over the backends
    pub fn iter(&self) -> std::slice::Iter<CacheBackend<'a>> {
        self.backends.iter()
    }

    /// Get a mutable iterator over the backends
    pub fn iter_mut(&mut self) -> std::slice::IterMut<CacheBackend<'a>> {
        self.backends.iter_mut()
    }
}

/// From implementations for ergonomic CacheBackend creation
impl<'a> From<deadpool_redis::Pool> for CacheBackend<'a> {
    fn from(pool: deadpool_redis::Pool) -> Self { CacheBackend::Redis(pool) }
}

impl<'a> From<(Cache<String, Bytes>, crate::config::MemoryConfig)>
    for CacheBackend<'a>
{
    fn from(
        (cache, config): (Cache<String, Bytes>, crate::config::MemoryConfig),
    ) -> Self {
        CacheBackend::Memory { cache, config }
    }
}

#[cfg(feature = "file-cache")]
impl<'a>
    From<(
        std::sync::Arc<tokio::sync::RwLock<sled::Db>>,
        crate::config::FileConfig,
    )> for CacheBackend<'a>
{
    fn from(
        (file_db, config): (
            std::sync::Arc<tokio::sync::RwLock<sled::Db>>,
            crate::config::FileConfig,
        ),
    ) -> Self {
        CacheBackend::File { file_db, config }
    }
}

/// Represents different cache backend types
pub enum CacheBackend<'a> {
    /// Redis backend using a deadpool connection pool
    Redis(deadpool_redis::Pool),

    /// In-memory cache backend
    Memory {
        cache: Cache<String, Bytes>,
        config: crate::config::MemoryConfig,
    },

    /// File-based cache backend
    #[cfg(feature = "file-cache")]
    File {
        file_db: std::sync::Arc<tokio::sync::RwLock<sled::Db>>,
        config: crate::config::FileConfig,
    },

    /// Tiered cache with multiple ordered backends
    /// Backends are checked in order: first = fastest, last = most persistent
    /// Example: [Memory, File, Redis] checks memory first, then file, then
    /// Redis The capacity is configurable via TieredConfig.max_layers
    Tiered {
        /// Ordered list of cache backends from fastest to most persistent
        /// Capacity is controlled by the TieredConfig.max_layers setting
        backends: BoundedBackends<'a>,
        /// Configuration for tiered behavior
        config: crate::config::TieredConfig,
    },
}

impl<'a> CacheBackend<'a> {
    /// Check if this is a Redis backend
    pub fn is_redis(&self) -> bool { matches!(self, CacheBackend::Redis(_)) }

    /// Create a tiered cache with the given backends
    /// Uses default configuration which determines the maximum capacity
    pub fn tiered(backends: Vec<CacheBackend<'a>>) -> Result<Self, String> {
        let config = crate::config::TieredConfig::default();
        Self::tiered_with_config(backends, config)
    }

    /// Create a tiered cache with custom configuration
    /// The configuration determines the maximum capacity for backends
    pub fn tiered_with_config(
        backends: Vec<CacheBackend<'a>>, config: crate::config::TieredConfig,
    ) -> Result<Self, String> {
        // Validate the backend count meets configuration requirements
        config.validate_backend_count(backends.len())?;

        // Create bounded backends with the configured capacity
        let mut bounded_backends =
            BoundedBackends::with_capacity(config.max_layers);

        // Add all backends to the bounded collection
        for backend in backends {
            bounded_backends.push(backend).map_err(|_| {
                "Too many backends provided for the configured capacity"
                    .to_string()
            })?;
        }

        Ok(CacheBackend::Tiered {
            backends: bounded_backends,
            config,
        })
    }

    /// Create a tiered cache builder that allows adding backends one by one
    /// This is useful when you want to build the cache incrementally
    pub fn tiered_builder(
        config: crate::config::TieredConfig,
    ) -> TieredCacheBuilder<'a> {
        TieredCacheBuilder::new(config)
    }

    /// Get the number of backends in a tiered cache
    pub fn backend_count(&self) -> Option<usize> {
        match self {
            CacheBackend::Tiered { backends, .. } => Some(backends.len()),
            _ => None,
        }
    }

    /// Check if this backend can handle the given number of layers
    pub fn can_handle_layers(&self, count: usize) -> bool {
        match self {
            CacheBackend::Tiered { config, .. } => {
                count >= config.min_layers && count <= config.max_layers
            }
            _ => count == 1, // Non-tiered backends can only handle 1 layer
        }
    }
}

/// Builder for creating tiered caches with validation
/// Provides a fluent API for constructing tiered caches layer by layer
pub struct TieredCacheBuilder<'a> {
    backends: BoundedBackends<'a>,
    config: crate::config::TieredConfig,
}

impl<'a> TieredCacheBuilder<'a> {
    /// Create a new builder with the given configuration
    pub fn new(config: crate::config::TieredConfig) -> Self {
        let backends = BoundedBackends::with_capacity(config.max_layers);
        Self { backends, config }
    }

    /// Add a backend layer to the tiered cache
    /// Layers are added in order from fastest to slowest
    pub fn add_layer(
        mut self, backend: CacheBackend<'a>,
    ) -> Result<Self, String> {
        self.backends.push(backend).map_err(|_| {
            format!("Cannot add more than {} layers", self.config.max_layers)
        })?;
        Ok(self)
    }

    /// Add a memory cache layer
    pub fn add_memory(
        self, cache: Cache<String, Bytes>,
        config: crate::config::MemoryConfig,
    ) -> Result<Self, String> {
        let backend = CacheBackend::Memory { cache, config };
        self.add_layer(backend)
    }

    /// Add a Redis cache layer
    pub fn add_redis(
        self, pool: deadpool_redis::Pool,
    ) -> Result<Self, String> {
        let backend = CacheBackend::Redis(pool);
        self.add_layer(backend)
    }

    /// Add a file cache layer
    #[cfg(feature = "file-cache")]
    pub fn add_file(
        self, file_db: std::sync::Arc<tokio::sync::RwLock<sled::Db>>,
        config: crate::config::FileConfig,
    ) -> Result<Self, String> {
        let backend = CacheBackend::File { file_db, config };
        self.add_layer(backend)
    }

    /// Add a file cache layer by opening a sled database at the given path
    #[cfg(feature = "file-cache")]
    pub fn add_file_at_path(
        self, path: std::path::PathBuf, config: crate::config::FileConfig,
    ) -> Result<Self, String> {
        let db = sled::open(&path).map_err(|e| {
            format!("Failed to open file cache at {}: {}", path.display(), e)
        })?;
        let file_db = std::sync::Arc::new(tokio::sync::RwLock::new(db));
        self.add_file(file_db, config)
    }

    /// Build the final tiered cache backend
    /// Validates that the minimum number of layers requirement is met
    pub fn build(self) -> Result<CacheBackend<'a>, String> {
        self.config.validate_backend_count(self.backends.len())?;

        Ok(CacheBackend::Tiered {
            backends: self.backends,
            config: self.config,
        })
    }

    /// Get the current number of layers
    pub fn layer_count(&self) -> usize { self.backends.len() }

    /// Get the maximum allowed layers
    pub fn max_layers(&self) -> usize { self.config.max_layers }

    /// Check if we can add more layers
    pub fn can_add_more(&self) -> bool {
        self.backends.len() < self.config.max_layers
    }
}
