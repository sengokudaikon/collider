use std::sync::OnceLock;

use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use tokio_postgres::NoTls;
use tracing::{info, instrument};

use crate::config::{DbConnectConfig, DbOptionsConfig, ReadReplicaConfig};

static SQL_DATABASE_POOL: OnceLock<Pool> = OnceLock::new();

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

    if let Some(max_conn) = config.max_conn() {
        pool_builder = pool_builder.max_size(max_conn as usize);
    }

    let pool = pool_builder.build()?;

    if SQL_DATABASE_POOL.set(pool).is_err() {
        panic!("SQL database pool already established")
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

        // Optimize read replica pool for higher concurrency
        if let Some(max_conn) = config.read_max_conn() {
            pool_builder = pool_builder.max_size(max_conn as usize);
        }
        else {
            // Default to higher connection count for read replicas
            pool_builder = pool_builder.max_size(800);
        }

        // Note: deadpool doesn't have wait_for_connections, but max_size
        // controls pool behavior

        let pool = pool_builder.build()?;

        if READ_DATABASE_POOL.set(pool).is_err() {
            panic!("Read replica database pool already established")
        }

        info!("Read replica connection pool initialized successfully");
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
