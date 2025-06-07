use std::convert::Infallible;

use database_traits::{
    connection::{FromRequestParts, GetDatabaseConnect, Parts},
    transaction::{GetDatabaseTransaction, TransactionOps},
};

#[derive(Debug, Clone)]
struct MockConnection;

#[derive(Debug, Clone)]
struct MockTransaction;

#[derive(Debug, Clone)]
struct MockConnect;

#[derive(Debug, thiserror::Error)]
enum MockError {
    #[error("Not found")]
    NotFound,
    #[error("Database error")]
    Database,
}

impl<S: Sync> FromRequestParts<S> for MockConnect {
    type Rejection = Infallible;

    async fn from_request_parts(
        _parts: &mut Parts, _state: &S,
    ) -> Result<Self, Self::Rejection> {
        Ok(MockConnect)
    }
}

impl GetDatabaseConnect for MockConnect {
    type Connect = MockConnection;

    fn get_connect(&self) -> &Self::Connect { &MockConnection }
}

impl GetDatabaseTransaction for MockConnect {
    type Error = MockError;
    type Transaction<'s> = MockTransaction;
    type TransactionFuture<'s> = std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<MockTransaction, MockError>,
                > + Send
                + 's,
        >,
    >;

    fn get_transaction(&self) -> Self::TransactionFuture<'_> {
        Box::pin(async { Ok(MockTransaction) })
    }
}

impl TransactionOps for MockTransaction {
    type Error = MockError;
    type RollBackFuture<'r> = std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<(), MockError>>
                + Send
                + 'r,
        >,
    >;
    type SubmitFuture<'r> = std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<(), MockError>>
                + Send
                + 'r,
        >,
    >;

    fn submit<'s>(self) -> Self::SubmitFuture<'s>
    where
        Self: 's,
    {
        Box::pin(async { Ok(()) })
    }

    fn rollback<'r>(self) -> Self::RollBackFuture<'r>
    where
        Self: 'r,
    {
        Box::pin(async { Ok(()) })
    }
}

#[tokio::test]
async fn test_connection_traits() {
    let connect = MockConnect;

    let _connection = connect.get_connect();

    let transaction = connect.get_transaction().await.unwrap();

    transaction.submit().await.unwrap();
}

#[tokio::test]
async fn test_transaction_rollback() {
    let connect = MockConnect;
    let transaction = connect.get_transaction().await.unwrap();

    transaction.rollback().await.unwrap();
}

#[tokio::test]
async fn test_error_trait_bounds() {
    let error = MockError::NotFound;

    assert_eq!(error.to_string(), "Not found");

    let error = MockError::Database;
    assert_eq!(error.to_string(), "Database error");
}

#[tokio::test]
async fn test_concurrent_transactions() {
    let connect = MockConnect;

    let tasks = (0..10).map(|_| {
        let connect = connect.clone();
        tokio::spawn(async move {
            let transaction = connect.get_transaction().await.unwrap();
            transaction.submit().await.unwrap();
        })
    });

    for task in tasks {
        task.await.unwrap();
    }
}

#[tokio::test]
async fn test_transaction_lifecycle() {
    let connect = MockConnect;

    let transaction1 = connect.get_transaction().await.unwrap();
    transaction1.submit().await.unwrap();

    let transaction2 = connect.get_transaction().await.unwrap();
    transaction2.rollback().await.unwrap();
}

#[tokio::test]
async fn test_multiple_connections() {
    let connect1 = MockConnect;
    let connect2 = MockConnect;

    let conn1 = connect1.get_connect();
    let conn2 = connect2.get_connect();

    assert!(std::ptr::eq(conn1, &MockConnection));
    assert!(std::ptr::eq(conn2, &MockConnection));
}

#[tokio::test]
async fn test_trait_object_compatibility() {
    let connect = MockConnect;

    let get_db_connect: &dyn GetDatabaseConnect<Connect = MockConnection> =
        &connect;
    let _connection = get_db_connect.get_connect();
    
    let transaction = connect.get_transaction().await.unwrap();
    transaction.submit().await.unwrap();
}
