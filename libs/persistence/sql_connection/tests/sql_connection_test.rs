use sql_connection::SqlConnect;
use test_utils::TestPostgresContainer;

#[tokio::test]
async fn test_sql_connect_creation() {
    let container = TestPostgresContainer::new().await.unwrap();
    let pool = container.pool.clone();

    let sql_connect = SqlConnect::new(pool);

    // Test that we can get a client
    let client = sql_connect.get_client().await;
    assert!(client.is_ok());
}

#[tokio::test]
async fn test_sql_connect_clone() {
    let container = TestPostgresContainer::new().await.unwrap();
    let pool = container.pool.clone();

    let sql_connect1 = SqlConnect::new(pool);
    let sql_connect2 = sql_connect1.clone();

    // Both should be able to get clients
    let client1 = sql_connect1.get_client().await;
    let client2 = sql_connect2.get_client().await;

    assert!(client1.is_ok());
    assert!(client2.is_ok());
}

#[tokio::test]
async fn test_database_operations() {
    let container = TestPostgresContainer::new().await.unwrap();
    let pool = container.pool.clone();
    let sql_connect = SqlConnect::new(pool);

    let client = sql_connect.get_client().await.unwrap();

    // Test basic query
    let result = client.query("SELECT 1 as test_value", &[]).await;
    assert!(result.is_ok());

    let rows = result.unwrap();
    assert_eq!(rows.len(), 1);

    let value: i32 = rows[0].get("test_value");
    assert_eq!(value, 1);
}

#[tokio::test]
async fn test_multiple_connections() {
    let container = TestPostgresContainer::new().await.unwrap();
    let pool = container.pool.clone();
    let sql_connect = SqlConnect::new(pool);

    // Get multiple clients from the same connection
    let client1 = sql_connect.get_client().await.unwrap();
    let client2 = sql_connect.get_client().await.unwrap();

    // Both should work independently
    let result1 = client1
        .query("SELECT 'client1' as source", &[])
        .await
        .unwrap();
    let result2 = client2
        .query("SELECT 'client2' as source", &[])
        .await
        .unwrap();

    let source1: String = result1[0].get("source");
    let source2: String = result2[0].get("source");

    assert_eq!(source1, "client1");
    assert_eq!(source2, "client2");
}

#[tokio::test]
async fn test_concurrent_operations() {
    let container = TestPostgresContainer::new().await.unwrap();
    let pool = container.pool.clone();
    let sql_connect = SqlConnect::new(pool);

    // Create multiple concurrent operations
    let mut handles = vec![];

    for i in 0..5 {
        let sql_connect_clone = sql_connect.clone();
        let handle = tokio::spawn(async move {
            let client = sql_connect_clone.get_client().await.unwrap();
            let result = client
                .query(&format!("SELECT {} as value", i), &[])
                .await
                .unwrap();
            let value: i32 = result[0].get("value");
            value
        });
        handles.push(handle);
    }

    // Wait for all operations to complete
    for (i, handle) in handles.into_iter().enumerate() {
        let result = handle.await.unwrap();
        assert_eq!(result, i as i32);
    }
}
