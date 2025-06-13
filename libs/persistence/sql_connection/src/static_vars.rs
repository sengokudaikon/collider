use std::sync::OnceLock;

use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use tokio_postgres::NoTls;
use tracing::{info, instrument};

use crate::config::{DbConnectConfig, DbOptionsConfig};

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

pub fn get_sql_pool() -> &'static Pool {
    SQL_DATABASE_POOL
        .get()
        .expect("SQL database pool not established")
}