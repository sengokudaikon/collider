pub trait DbConnectConfig: serde::de::DeserializeOwned {
    #[allow(unused)]
    fn password(&self) -> Option<&str> { None }
    fn host(&self) -> &str;
    fn port(&self) -> u16;
    fn db(&self) -> u8;
}

#[derive(Debug, serde::Deserialize)]
pub struct RedisDbConfig {
    #[serde(default = "host_default")]
    pub host: String,
    #[serde(default = "port_default")]
    pub port: u16,
    #[serde(default = "db_default")]
    pub db: u8,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct MemoryConfig {
    #[serde(default = "default_memory_capacity")]
    pub capacity: u64,
    #[serde(default = "default_memory_ttl_secs")]
    pub ttl_secs: u64,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TieredConfig {
    /// Maximum number of cache layers allowed (enforces resource limits)
    #[serde(default = "default_max_layers")]
    pub max_layers: usize,

    /// Minimum number of cache layers required for a valid tiered cache
    #[serde(default = "default_min_layers")]
    pub min_layers: usize,

    /// Strategy for when a layer is full or unavailable
    #[serde(default = "default_overflow_strategy")]
    pub overflow_strategy: OverflowStrategy,

    /// Write strategy: WriteThrough (write to all layers) or WriteBack
    /// (write to fastest, sync later)
    #[serde(default = "default_write_strategy")]
    pub write_strategy: WriteStrategy,

    /// Whether to populate faster layers on read hits from slower layers
    #[serde(default = "default_populate_on_read")]
    pub populate_on_read: bool,
}

#[derive(Debug, Clone, Copy, serde::Deserialize, PartialEq)]
pub enum WriteStrategy {
    /// Write to all cache layers immediately
    WriteThrough,
    /// Write to fastest layer, sync to others asynchronously
    WriteBack,
    /// Only write to the specified layer (usually the most persistent)
    WriteToSlowest,
}

#[cfg(feature = "file-cache")]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct FileConfig {
    pub path: std::path::PathBuf,
    #[serde(default = "default_file_cache_size")]
    pub max_size_mb: u64,
}

#[derive(Debug, Clone, Copy, serde::Deserialize, PartialEq)]
pub enum OverflowStrategy {
    Drop,
    MoveToRedis,
}

impl DbConnectConfig for RedisDbConfig {
    fn password(&self) -> Option<&str> { None }

    fn host(&self) -> &str { &self.host }

    fn port(&self) -> u16 { self.port }

    fn db(&self) -> u8 { self.db }
}

fn host_default() -> String { "127.0.0.1".into() }
fn port_default() -> u16 { 6379 }
fn db_default() -> u8 { 0 }
fn default_memory_capacity() -> u64 { 10_000 }
fn default_memory_ttl_secs() -> u64 { 300 }
fn default_max_layers() -> usize { 4 } // Reasonable default: Memory → File → Redis → Cold Storage
fn default_min_layers() -> usize { 2 } // At least 2 layers for tiering to make sense
fn default_overflow_strategy() -> OverflowStrategy { OverflowStrategy::Drop }
fn default_write_strategy() -> WriteStrategy { WriteStrategy::WriteThrough }
fn default_populate_on_read() -> bool { true }

#[cfg(feature = "file-cache")]
fn default_file_cache_size() -> u64 { 1024 } // 1GB default

impl MemoryConfig {
    pub fn ttl(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.ttl_secs)
    }
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            capacity: default_memory_capacity(),
            ttl_secs: default_memory_ttl_secs(),
        }
    }
}

impl Default for TieredConfig {
    fn default() -> Self {
        Self {
            max_layers: default_max_layers(),
            min_layers: default_min_layers(),
            overflow_strategy: default_overflow_strategy(),
            write_strategy: default_write_strategy(),
            populate_on_read: default_populate_on_read(),
        }
    }
}

impl TieredConfig {
    /// Validate that the number of backends meets the configuration
    /// requirements
    pub fn validate_backend_count(
        &self, backend_count: usize,
    ) -> Result<(), String> {
        if backend_count < self.min_layers {
            return Err(format!(
                "Tiered cache requires at least {} layers, but {} were \
                 provided",
                self.min_layers, backend_count
            ));
        }

        if backend_count > self.max_layers {
            return Err(format!(
                "Tiered cache allows at most {} layers, but {} were provided",
                self.max_layers, backend_count
            ));
        }

        Ok(())
    }
}
