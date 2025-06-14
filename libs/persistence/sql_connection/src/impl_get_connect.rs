use std::convert::Infallible;

use database_traits::connection::{
    FromRequestParts, Parts,
};
use deadpool_postgres::{Object, Pool};

use crate::static_vars::get_sql_pool;

#[derive(Debug, Clone)]
pub struct SqlConnect {
    pool: Pool,
}

impl SqlConnect {
    pub fn new(pool: Pool) -> Self { Self { pool } }

    pub fn from_global() -> Self {
        Self {
            pool: get_sql_pool().clone(),
        }
    }

    pub async fn get_client(
        &self,
    ) -> Result<Object, deadpool_postgres::PoolError> {
        self.pool.get().await
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
