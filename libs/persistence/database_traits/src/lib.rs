use std::{future::Future, pin::Pin};

pub mod connection;
pub mod dao;
pub mod transaction;

pub type BoxedResultFuture<'r, T, E> =
    Pin<Box<dyn Future<Output = Result<T, E>> + 'r>>;
pub type BoxedResultSendFuture<'r, T, E> =
    Pin<Box<dyn Future<Output = Result<T, E>> + 'r + Send>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_aliases() {
        use std::marker::PhantomData;
        let _phantom: PhantomData<BoxedResultFuture<'_, (), ()>> =
            PhantomData;
        let _phantom: PhantomData<BoxedResultSendFuture<'_, (), ()>> =
            PhantomData;
    }
}
