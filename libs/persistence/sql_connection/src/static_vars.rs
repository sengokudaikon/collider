use std::sync::OnceLock;

use sea_orm::{
    ConnectOptions, Database, DatabaseConnection, DatabaseTransaction, DbErr,
    TransactionTrait,
};
use tracing::{info, instrument, log};

use crate::config::{DbConnectConfig, DbOptionsConfig};

static SQL_DATABASE_CONNECTION: OnceLock<DatabaseConnection> =
    OnceLock::new();

#[instrument(skip_all, name = "connect-pgsql")]
pub async fn connect_postgres_db<C>(config: &C) -> Result<(), DbErr>
where
    C: DbConnectConfig + DbOptionsConfig,
{
    let db_url = config.uri();

    info!(
        mysql.url = db_url,
        mysql.max_conn = ?config.max_conn(),
        mysql.min_conn = ?config.min_conn(),
        mysql.sqlx.log = config.sql_logger()
    );

    let mut db_options = ConnectOptions::new(db_url);
    if let Some(max_conn) = config.max_conn() {
        db_options.max_connections(max_conn);
    }
    if let Some(min_conn) = config.min_conn() {
        db_options.min_connections(min_conn);
    }
    db_options
        .sqlx_logging(true)
        .sqlx_logging_level(log::LevelFilter::Info)
        .set_schema_search_path("public");

    let connect = Database::connect(db_options).await?;

    if SQL_DATABASE_CONNECTION.set(connect).is_err() {
        panic!("SQL database connection already established")
    }
    Ok(())
}

pub fn get_sql_database() -> &'static DatabaseConnection {
    SQL_DATABASE_CONNECTION
        .get()
        .expect("SQL database connection not established")
}

#[allow(dead_code)]
pub async fn get_sql_transaction() -> Result<DatabaseTransaction, DbErr> {
    get_sql_database().begin().await
}
