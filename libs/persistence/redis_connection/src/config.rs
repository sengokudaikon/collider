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
    pub memory: MemoryConfig,
    #[serde(default = "default_overflow_strategy")]
    pub overflow_strategy: OverflowStrategy,
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
fn default_overflow_strategy() -> OverflowStrategy { OverflowStrategy::Drop }

impl MemoryConfig {
    pub fn ttl(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.ttl_secs)
    }
}
