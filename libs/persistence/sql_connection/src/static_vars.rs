use std::{sync::OnceLock, time::Duration};

use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use tokio_postgres::NoTls;
use tracing::{debug, info, instrument};

use crate::config::{DbConnectConfig, DbOptionsConfig, ReadReplicaConfig};

static SQL_DATABASE_POOL: OnceLock<Pool> = OnceLock::new();

/// Pre-warms a connection pool by creating connections up front
async fn prewarm_pool(pool: &Pool, count: u32) {
    debug!("Pre-warming pool with {} connections", count);
    let mut handles = vec![];

    for i in 0..count {
        let pool = pool.clone();
        handles.push(tokio::spawn(async move {
            match pool.get().await {
                Ok(_conn) => {
                    debug!("Pre-warmed connection {}/{}", i + 1, count);
                    // Connection automatically returns to pool when dropped
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to pre-warm connection {}: {}",
                        i + 1,
                        e
                    );
                }
            }
        }));
    }

    // Wait for all connections to be established
    for handle in handles {
        let _ = handle.await;
    }

    // Give connections time to settle in the pool
    tokio::time::sleep(Duration::from_millis(100)).await;

    let status = pool.status();
    info!(
        "Pool pre-warming complete: {} connections available",
        status.available
    );
}

#[instrument(skip_all, name = "connect-pgsql")]
pub async fn connect_postgres_db<C>(config: &C) -> Result<(), anyhow::Error>
where
    C: DbConnectConfig + DbOptionsConfig,
{
    let db_url = config.uri();

    info!(
        postgres.url = db_url,
        postgres.max_conn = ?config.max_conn(),
        postgres.min_conn = ?config.min_conn(),
        postgres.sql_logger = config.sql_logger()
    );

    let pg_config = db_url.parse::<tokio_postgres::Config>()?;

    let mgr_config = ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
    };
    let mgr = Manager::from_config(pg_config, NoTls, mgr_config);

    let mut pool_builder = Pool::builder(mgr);

    // Configure runtime and timeouts for better burst performance
    pool_builder = pool_builder
        .runtime(deadpool_postgres::Runtime::Tokio1) // Required for timeout support
        .wait_timeout(Some(Duration::from_millis(2000))) // Wait max 2s for connection
        .create_timeout(Some(Duration::from_millis(5000))) // Create connection within 5s
        .recycle_timeout(Some(Duration::from_millis(100))); // Fast recycling

    if let Some(max_conn) = config.max_conn() {
        pool_builder = pool_builder.max_size(max_conn as usize);
    }

    let pool = pool_builder.build()?;

    if SQL_DATABASE_POOL.set(pool.clone()).is_err() {
        panic!("SQL database pool already established")
    }

    // Pre-warm the connection pool
    if let Some(min_conn) = config.min_conn() {
        info!("Pre-warming primary pool with {} connections", min_conn);
        prewarm_pool(&pool, min_conn).await;
    }

    Ok(())
}

static READ_DATABASE_POOL: OnceLock<Pool> = OnceLock::new();

#[instrument(skip_all, name = "connect-pgsql-read-replica")]
pub async fn connect_postgres_read_replica<C>(
    config: &C,
) -> Result<(), anyhow::Error>
where
    C: DbConnectConfig + DbOptionsConfig,
    C: ReadReplicaConfig,
{
    if let Some(read_uri) = config.read_replica_uri() {
        info!(
            postgres.read_replica.url = read_uri,
            postgres.read_replica.max_conn = ?config.read_max_conn(),
            postgres.read_replica.min_conn = ?config.read_min_conn(),
            "Setting up read replica connection pool"
        );

        let pg_config = read_uri.parse::<tokio_postgres::Config>()?;

        let mgr_config = ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        };
        let mgr = Manager::from_config(pg_config, NoTls, mgr_config);

        let mut pool_builder = Pool::builder(mgr);

        // Configure runtime and timeouts for better burst performance
        pool_builder = pool_builder
            .runtime(deadpool_postgres::Runtime::Tokio1) // Required for timeout support
            .wait_timeout(Some(Duration::from_millis(2000))) // Wait max 2s for connection
            .create_timeout(Some(Duration::from_millis(5000))) // Create connection within 5s
            .recycle_timeout(Some(Duration::from_millis(100))); // Fast recycling

        // Optimize read replica pool for higher concurrency
        if let Some(max_conn) = config.read_max_conn() {
            pool_builder = pool_builder.max_size(max_conn as usize);
        }
        else {
            // Default to higher connection count for read replicas
            pool_builder = pool_builder.max_size(800);
        }

        let pool = pool_builder.build()?;

        if READ_DATABASE_POOL.set(pool.clone()).is_err() {
            panic!("Read replica database pool already established")
        }

        info!("Read replica connection pool initialized successfully");

        // Pre-warm the read replica pool
        let prewarm_count = config.read_min_conn().unwrap_or(10);
        info!(
            "Pre-warming read replica pool with {} connections",
            prewarm_count
        );
        prewarm_pool(&pool, prewarm_count).await;
    }
    Ok(())
}

pub fn get_read_sql_pool() -> Option<&'static Pool> {
    READ_DATABASE_POOL.get()
}
pub fn get_sql_pool() -> &'static Pool {
    SQL_DATABASE_POOL
        .get()
        .expect("SQL database pool not established")
}
