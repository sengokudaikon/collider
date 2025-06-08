use deadpool_redis::{Config, CreatePoolError, Pool, Runtime};
pub use deadpool_redis::{PoolError, redis::FromRedisValue};
pub use redis::{AsyncCommands, RedisError};
use tracing::{info, instrument};
use url::Url;
pub mod config;
pub mod connection;
pub mod hash;
pub mod json;
pub mod key;
pub mod macros;
pub mod memory;
pub mod normal;
pub mod redis_value;
pub mod tiered;
pub mod type_bind;

#[instrument(skip_all, name = "connect-dragonfly")]
pub async fn connect_redis_db<C>(config: &C) -> Result<Pool, CreatePoolError>
where
    C: config::DbConnectConfig,
{
    let mut url = Url::parse("redis://").unwrap();

    url.set_host(Some(config.host())).unwrap();
    url.set_port(config.port().into()).unwrap();
    url.path_segments_mut()
        .unwrap()
        .extend(&[config.db().to_string()]);

    info!(redis.url = %url, redis.connect = true);

    let cfg = Config {
        url: Some(url.to_string()),
        pool: Some(deadpool_redis::PoolConfig::default()),
        connection: None,
    };

    let pool = cfg.create_pool(Some(Runtime::Tokio1))?;
    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_construction() {
        #[derive(serde::Deserialize)]
        struct TestConfig {
            host: String,
            port: u16,
            db: u8,
        }

        impl config::DbConnectConfig for TestConfig {
            fn host(&self) -> &str { &self.host }

            fn port(&self) -> u16 { self.port }

            fn db(&self) -> u8 { self.db }
        }

        let mut url = Url::parse("redis://").unwrap();
        let config = TestConfig {
            host: "localhost".to_string(),
            port: 6379,
            db: 0,
        };

        url.set_host(Some(config::DbConnectConfig::host(&config)))
            .unwrap();
        url.set_port(config::DbConnectConfig::port(&config).into())
            .unwrap();
        url.path_segments_mut()
            .unwrap()
            .extend(&[config::DbConnectConfig::db(&config).to_string()]);

        assert_eq!(url.to_string(), "redis://localhost:6379/0");
    }

    #[test]
    fn test_redis_db_config_default() {
        use config::RedisDbConfig;

        let json = r#"{}"#;
        let config: RedisDbConfig = serde_json::from_str(json).unwrap();

        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 6379);
        assert_eq!(config.db, 0);
    }
}
