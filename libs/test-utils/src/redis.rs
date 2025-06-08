use std::time::Duration;

use deadpool_redis::{Config, Pool, Runtime};
use testcontainers::{ContainerAsync, runners::AsyncRunner};
use testcontainers_modules::redis::Redis;
use tokio::time::sleep;

pub struct TestRedisContainer {
    #[allow(dead_code)]
    container: ContainerAsync<Redis>,
    pub pool: Pool,
    pub connection_string: String,
}

impl TestRedisContainer {
    pub async fn new() -> anyhow::Result<Self> {
        let container = Redis::default().start().await?;

        let host = container.get_host().await?;
        let port = container.get_host_port_ipv4(6379).await?;

        let connection_string = format!("redis://{}:{}", host, port);

        sleep(Duration::from_secs(2)).await;

        let mut cfg = Config::from_url(&connection_string);
        cfg.pool = Some(deadpool_redis::PoolConfig::new(10));
        let pool = cfg.create_pool(Some(Runtime::Tokio1))?;

        let mut attempts = 0;
        loop {
            match pool.get().await {
                Ok(mut conn) => {
                    match deadpool_redis::redis::cmd("PING")
                        .query_async::<()>(&mut conn)
                        .await
                    {
                        Ok(_) => break,
                        Err(_) if attempts < 10 => {
                            attempts += 1;
                            sleep(Duration::from_millis(500 * attempts))
                                .await;
                            continue;
                        }
                        Err(e) => return Err(e.into()),
                    }
                }
                Err(_) if attempts < 10 => {
                    attempts += 1;
                    sleep(Duration::from_millis(500 * attempts)).await;
                    continue;
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(Self {
            container,
            pool,
            connection_string,
        })
    }

    pub async fn get_connection(
        &self,
    ) -> anyhow::Result<deadpool_redis::Connection> {
        Ok(self.pool.get().await?)
    }
}
