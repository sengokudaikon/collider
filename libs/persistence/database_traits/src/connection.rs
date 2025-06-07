use std::{error::Error as StdError, sync::Arc};

pub use axum_core::extract::{FromRef, FromRequestParts};
pub use http::request::Parts;

pub trait GetDatabaseConnect {
    type Connect;
    fn get_connect(&self) -> &Self::Connect;
}

pub trait GetMutDatabaseConnect {
    type Connect;
    fn mut_connect(&mut self) -> &mut Self::Connect;
}

pub type CollectionResult<'s, Conn, C> = Result<
    <Conn as GetDatabaseCollection<C>>::CollectGuard<'s>,
    <Conn as GetDatabaseCollection<C>>::Error,
>;

pub trait GetDatabaseCollection<C>: GetDatabaseConnect {
    type Error: StdError;
    type CollectGuard<'s>: 's
    where
        Self: 's;
    fn get_collection(&self) -> Result<Self::CollectGuard<'_>, Self::Error>;
}

impl<T> GetDatabaseConnect for Arc<T>
where
    T: GetDatabaseConnect,
{
    type Connect = T::Connect;

    fn get_connect(&self) -> &Self::Connect { (**self).get_connect() }
}
