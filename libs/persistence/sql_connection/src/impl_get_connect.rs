use core::{future::Future, marker::Send, pin::Pin};
use std::{
    convert::Infallible,
    ops::{Deref, DerefMut},
};

use database_traits::{
    BoxedResultSendFuture,
    connection::{FromRequestParts, GetDatabaseConnect, Parts},
    transaction::{GetDatabaseTransaction, TransactionOps},
};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbErr,
    StreamTrait, TransactionStream, TransactionTrait,
};

use crate::static_vars::get_sql_database;

#[derive(Debug, Clone)]
pub struct SqlConnect {
    connection: DatabaseConnection,
}

impl SqlConnect {
    pub fn new(connection: DatabaseConnection) -> Self { Self { connection } }

    pub fn from_global() -> Self {
        Self {
            connection: get_sql_database().clone(),
        }
    }
}

impl Default for SqlConnect {
    fn default() -> Self { Self::from_global() }
}

impl<S> FromRequestParts<S> for SqlConnect {
    type Rejection = Infallible;

    fn from_request_parts(
        _parts: &mut Parts, _state: &S,
    ) -> impl std::future::Future<
        Output = Result<Self, <Self as FromRequestParts<S>>::Rejection>,
    > + Send {
        Box::pin(async { Ok(SqlConnect::from_global()) })
    }
}

impl GetDatabaseConnect for SqlConnect {
    type Connect = DatabaseConnection;

    fn get_connect(&self) -> &Self::Connect { &self.connection }
}

#[derive(Debug)]
pub struct SqlTransaction(pub DatabaseTransaction);

impl GetDatabaseTransaction for SqlConnect {
    type Error = DbErr;
    type Transaction<'s> = SqlTransaction;
    type TransactionFuture<'s> =
        BoxedResultSendFuture<'s, SqlTransaction, DbErr>;

    fn get_transaction(&self) -> Self::TransactionFuture<'_> {
        let conn = self.connection.clone();
        Box::pin(async move { conn.begin().await.map(SqlTransaction) })
    }
}

impl TransactionOps for SqlTransaction {
    type Error = DbErr;
    type RollBackFuture<'r> = BoxedResultSendFuture<'r, (), DbErr>;
    type SubmitFuture<'r> = BoxedResultSendFuture<'r, (), DbErr>;

    fn submit<'s>(self) -> Self::SubmitFuture<'s>
    where
        Self: 's,
    {
        Box::pin(self.0.commit())
    }

    fn rollback<'r>(self) -> Self::RollBackFuture<'r>
    where
        Self: 'r,
    {
        Box::pin(self.0.rollback())
    }
}

impl Deref for SqlTransaction {
    type Target = DatabaseTransaction;

    fn deref(&self) -> &Self::Target { &self.0 }
}

impl DerefMut for SqlTransaction {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}

impl ConnectionTrait for SqlTransaction {
    fn get_database_backend(&self) -> sea_orm::DbBackend {
        self.0.get_database_backend()
    }

    fn execute<'life0, 'async_trait>(
        &'life0 self, stmt: sea_orm::Statement,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<sea_orm::ExecResult, DbErr>>
                + Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        self.0.execute(stmt)
    }

    fn execute_unprepared<'life0, 'life1, 'async_trait>(
        &'life0 self, sql: &'life1 str,
    ) -> core::pin::Pin<
        Box<
            dyn core::future::Future<
                    Output = Result<sea_orm::ExecResult, DbErr>,
                > + core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        self.0.execute_unprepared(sql)
    }

    fn query_one<'life0, 'async_trait>(
        &'life0 self, stmt: sea_orm::Statement,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Option<sea_orm::QueryResult>, DbErr>>
                + Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        self.0.query_one(stmt)
    }

    fn query_all<'life0, 'async_trait>(
        &'life0 self, stmt: sea_orm::Statement,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<sea_orm::QueryResult>, DbErr>>
                + Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        self.0.query_all(stmt)
    }
}

impl StreamTrait for SqlTransaction {
    type Stream<'a> = TransactionStream<'a>;

    fn stream<'a>(
        &'a self, stmt: sea_orm::Statement,
    ) -> std::pin::Pin<
        Box<dyn Future<Output = Result<Self::Stream<'a>, DbErr>> + 'a + Send>,
    > {
        self.0.stream(stmt)
    }
}

impl TransactionTrait for SqlTransaction {
    fn begin<'life0, 'async_trait>(
        &'life0 self,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<DatabaseTransaction, DbErr>>
                + Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        self.0.begin()
    }

    fn transaction<'life0, 'async_trait, F, T, E>(
        &'life0 self, callback: F,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<T, sea_orm::TransactionError<E>>>
                + Send
                + 'async_trait,
        >,
    >
    where
        F: for<'c> FnOnce(
                &'c DatabaseTransaction,
            ) -> std::pin::Pin<
                Box<dyn Future<Output = Result<T, E>> + Send + 'c>,
            > + Send,
        T: Send,
        E: std::fmt::Display + std::fmt::Debug + Send,
        F: 'async_trait,
        T: 'async_trait,
        E: 'async_trait,
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        self.0.transaction(callback)
    }

    fn begin_with_config<'life0, 'async_trait>(
        &'life0 self, isolation_level: Option<sea_orm::IsolationLevel>,
        access_mode: Option<sea_orm::AccessMode>,
    ) -> core::pin::Pin<
        Box<
            dyn core::future::Future<
                    Output = Result<DatabaseTransaction, DbErr>,
                > + core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        self.0.begin_with_config(isolation_level, access_mode)
    }

    fn transaction_with_config<'life0, 'async_trait, F, T, E>(
        &'life0 self, callback: F,
        isolation_level: Option<sea_orm::IsolationLevel>,
        access_mode: Option<sea_orm::AccessMode>,
    ) -> core::pin::Pin<
        Box<
            dyn core::future::Future<
                    Output = Result<T, sea_orm::TransactionError<E>>,
                > + core::marker::Send
                + 'async_trait,
        >,
    >
    where
        F: for<'c> FnOnce(
                &'c DatabaseTransaction,
            ) -> Pin<
                Box<dyn Future<Output = Result<T, E>> + Send + 'c>,
            > + Send,
        T: Send,
        E: std::fmt::Display + std::fmt::Debug + Send,
        F: 'async_trait,
        T: 'async_trait,
        E: 'async_trait,
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        self.0
            .transaction_with_config(callback, isolation_level, access_mode)
    }
}
