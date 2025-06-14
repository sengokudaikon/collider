use std::{future::Future, sync::Arc};

use super::connection::GetDatabaseConnect;

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
