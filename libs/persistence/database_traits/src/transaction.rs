use std::{
    future::Future,
    ops::{Deref, DerefMut},
    pin::Pin,
    sync::Arc,
};

use sea_orm::{
    ConnectionTrait, DatabaseTransaction, DbErr, TransactionTrait,
};

use super::{BoxedResultSendFuture, connection::GetDatabaseConnect};

pub trait TransactionOps {
    type Error: std::error::Error;

    type SubmitFuture<'s>: Future<Output = Result<(), Self::Error>>
        + 's
        + Send
    where
        Self: 's;

    fn submit<'s>(self) -> Self::SubmitFuture<'s>
    where
        Self: 's;

    type RollBackFuture<'r>: Future<Output = Result<(), Self::Error>>
        + 'r
        + Send
    where
        Self: 'r;
    #[allow(unused)]
    fn rollback<'r>(self) -> Self::RollBackFuture<'r>
    where
        Self: 'r;
}

pub trait GetDatabaseTransaction: GetDatabaseConnect {
    type Error: std::error::Error;
    type Transaction<'s>: TransactionOps<Error = Self::Error> + 's
    where
        Self: 's;

    type TransactionFuture<'s>: Future<Output = Result<Self::Transaction<'s>, Self::Error>>
        + Send
        + 's
    where
        Self: 's;

    fn get_transaction(&self) -> Self::TransactionFuture<'_>;
}
#[derive(Debug)]
pub struct SqlTransaction(pub DatabaseTransaction);

impl<T> GetDatabaseTransaction for Arc<T>
where
    T: GetDatabaseTransaction + Send + Sync,
{
    type Error = T::Error;
    type Transaction<'s>
        = T::Transaction<'s>
    where
        Self: 's;
    type TransactionFuture<'s>
        = T::TransactionFuture<'s>
    where
        Self: 's;

    fn get_transaction(&self) -> Self::TransactionFuture<'_> {
        (**self).get_transaction()
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
    ) -> Pin<
        Box<
            dyn Future<Output = Result<sea_orm::ExecResult, DbErr>>
                + Send
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

    fn begin_with_config<'life0, 'async_trait>(
        &'life0 self, isolation_level: Option<sea_orm::IsolationLevel>,
        access_mode: Option<sea_orm::AccessMode>,
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
        self.0.begin_with_config(isolation_level, access_mode)
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
        self.0.transaction(callback)
    }

    fn transaction_with_config<'life0, 'async_trait, F, T, E>(
        &'life0 self, callback: F,
        isolation_level: Option<sea_orm::IsolationLevel>,
        access_mode: Option<sea_orm::AccessMode>,
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
