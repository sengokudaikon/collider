pub use config::{DbConnectConfig, DbOptionsConfig, PostgresDbConfig};
pub use database_traits;
pub use deadpool_postgres::PoolError;
pub use impl_get_connect::SqlConnect;
pub use tokio_postgres::Error as PgError;
pub mod config;
mod impl_get_connect;
mod static_vars;

pub use static_vars::{
    connect_postgres_db, connect_postgres_read_replica, get_sql_pool,
};
