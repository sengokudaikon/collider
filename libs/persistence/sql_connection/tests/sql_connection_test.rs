use sea_orm::{ConnectionTrait, Statement, TransactionTrait};
use test_utils::postgres::TestPostgresContainer;

async fn setup_test_connection() -> anyhow::Result<TestPostgresContainer> {
    TestPostgresContainer::new().await
}

#[tokio::test]
async fn test_sql_connect_with_test_database() {
    let container = setup_test_connection().await.unwrap();

    let backend = container.connection.get_database_backend();
    assert!(matches!(backend, sea_orm::DatabaseBackend::Postgres));
}

#[tokio::test]
async fn test_database_connection_execute() {
    let container = setup_test_connection().await.unwrap();

    let result = container
        .connection
        .execute(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            "SELECT 1 as test_value".to_string(),
        ))
        .await;

    assert!(result.is_ok());
    let exec_result = result.unwrap();
    assert_eq!(exec_result.rows_affected(), 1);
}

#[tokio::test]
async fn test_database_connection_query_one() {
    let container = setup_test_connection().await.unwrap();

    let result = container
        .connection
        .query_one(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            "SELECT 42 as answer".to_string(),
        ))
        .await;

    assert!(result.is_ok());
    let query_result = result.unwrap();
    assert!(query_result.is_some());

    let row = query_result.unwrap();
    let value: i32 = row.try_get("", "answer").unwrap();
    assert_eq!(value, 42);
}

#[tokio::test]
async fn test_database_connection_query_all() {
    let container = setup_test_connection().await.unwrap();

    container
        .execute_sql(
            "CREATE TABLE test_table (id SERIAL PRIMARY KEY, value INTEGER)",
        )
        .await
        .unwrap();

    container
        .execute_sql("INSERT INTO test_table (value) VALUES (1), (2), (3)")
        .await
        .unwrap();

    let result = container
        .connection
        .query_all(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            "SELECT value FROM test_table ORDER BY value".to_string(),
        ))
        .await;

    assert!(result.is_ok());
    let rows = result.unwrap();
    assert_eq!(rows.len(), 3);

    let values: Vec<i32> = rows
        .iter()
        .map(|row| row.try_get("", "value").unwrap())
        .collect();
    assert_eq!(values, vec![1, 2, 3]);
}

#[tokio::test]
async fn test_transaction_commit() {
    let container = setup_test_connection().await.unwrap();

    container
        .execute_sql(
            "CREATE TABLE tx_test (id SERIAL PRIMARY KEY, value INTEGER)",
        )
        .await
        .unwrap();

    let txn = container.connection.begin().await.unwrap();

    txn.execute(Statement::from_string(
        sea_orm::DatabaseBackend::Postgres,
        "INSERT INTO tx_test (value) VALUES (100)".to_string(),
    ))
    .await
    .unwrap();

    txn.commit().await.unwrap();

    let result = container
        .connection
        .query_one(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            "SELECT COUNT(*) as count FROM tx_test WHERE value = 100"
                .to_string(),
        ))
        .await
        .unwrap()
        .unwrap();

    let count: i64 = result.try_get("", "count").unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn test_transaction_rollback() {
    let container = setup_test_connection().await.unwrap();

    container
        .execute_sql(
            "CREATE TABLE tx_rollback_test (id SERIAL PRIMARY KEY, value \
             INTEGER)",
        )
        .await
        .unwrap();

    let txn = container.connection.begin().await.unwrap();

    txn.execute(Statement::from_string(
        sea_orm::DatabaseBackend::Postgres,
        "INSERT INTO tx_rollback_test (value) VALUES (200)".to_string(),
    ))
    .await
    .unwrap();

    txn.rollback().await.unwrap();

    let result = container
        .connection
        .query_one(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            "SELECT COUNT(*) as count FROM tx_rollback_test WHERE value = \
             200"
            .to_string(),
        ))
        .await
        .unwrap()
        .unwrap();

    let count: i64 = result.try_get("", "count").unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_database_connection_multiple_queries() {
    let container = setup_test_connection().await.unwrap();

    for i in 1..=5 {
        let result = container
            .connection
            .query_one(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                format!("SELECT {} as number", i),
            ))
            .await;

        assert!(result.is_ok());
        let query_result = result.unwrap().unwrap();
        let value: i32 = query_result.try_get("", "number").unwrap();
        assert_eq!(value, i);
    }
}

#[tokio::test]
async fn test_database_backend_detection() {
    let container = setup_test_connection().await.unwrap();

    let backend = container.connection.get_database_backend();
    assert!(matches!(backend, sea_orm::DatabaseBackend::Postgres));
}

#[tokio::test]
async fn test_sequential_queries() {
    let container = setup_test_connection().await.unwrap();

    for i in 0..5 {
        let result = container
            .connection
            .query_one(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                format!("SELECT {} as sequential_value", i),
            ))
            .await;

        assert!(result.is_ok());
        let query_result = result.unwrap();
        assert!(query_result.is_some());

        let row = query_result.unwrap();
        let value: i32 = row.try_get("", "sequential_value").unwrap();
        assert_eq!(value, i);
    }
}

#[tokio::test]
async fn test_table_operations() {
    let container = setup_test_connection().await.unwrap();

    container
        .execute_sql(
            "CREATE TABLE operations_test (
                id SERIAL PRIMARY KEY, 
                name VARCHAR(100) NOT NULL,
                created_at TIMESTAMP DEFAULT NOW()
            )",
        )
        .await
        .unwrap();

    container
        .execute_sql(
            "INSERT INTO operations_test (name) VALUES ('test1'), ('test2')",
        )
        .await
        .unwrap();

    let result = container
        .connection
        .query_all(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            "SELECT id, name FROM operations_test ORDER BY id".to_string(),
        ))
        .await
        .unwrap();

    assert_eq!(result.len(), 2);

    let first_name: String = result[0].try_get("", "name").unwrap();
    let second_name: String = result[1].try_get("", "name").unwrap();

    assert_eq!(first_name, "test1");
    assert_eq!(second_name, "test2");
}
