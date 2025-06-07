use serde::Deserialize;
use sql_connection::SqlConnect;
use thiserror::Error;
use tracing::instrument;
use user_dao::UserDao;
use user_models as users;

#[derive(Debug, Error)]
pub enum ListUsersError {
    #[error("DAO error: {0}")]
    Dao(#[from] user_dao::UserDaoError),
}

#[derive(Debug, Deserialize)]
pub struct ListUsersQuery {
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

#[derive(Clone)]
pub struct ListUsersQueryHandler {
    user_dao: UserDao,
}

impl ListUsersQueryHandler {
    pub fn new(db: SqlConnect) -> Self {
        Self {
            user_dao: UserDao::new(db),
        }
    }

    #[instrument(skip(self))]
    pub async fn execute(
        &self, query: ListUsersQuery,
    ) -> Result<Vec<users::Model>, ListUsersError> {
        let users = self
            .user_dao
            .find_with_pagination(query.limit, query.offset)
            .await?;
        Ok(users)
    }
}
