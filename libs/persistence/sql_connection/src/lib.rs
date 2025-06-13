pub use config::{DbConnectConfig, DbOptionsConfig, PostgresDbConfig};
pub use database_traits;
pub use impl_get_connect::SqlConnect;

pub mod config;
mod impl_get_connect;
mod static_vars;

pub use static_vars::{connect_postgres_db, get_sql_pool};
