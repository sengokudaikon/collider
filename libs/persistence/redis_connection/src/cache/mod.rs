pub mod memory;
pub mod tiered;

#[cfg(feature = "file-cache")]
pub mod file_cache;
mod memory_standalone;
mod r#trait;

// Re-export the cache types
pub use memory::Memory;
pub use tiered::Tiered;

#[cfg(feature = "file-cache")]
pub use file_cache::FileCache;