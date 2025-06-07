use database_traits::dao::GenericDao;
use serde::Deserialize;
use sql_connection::SqlConnect;
use thiserror::Error;
use tracing::instrument;
use user_dao::UserDao;
use user_models as users;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum GetUserError {
    #[error("DAO error: {0}")]
    Dao(#[from] user_dao::UserDaoError),
    #[error("User not found: {user_id}")]
    NotFound { user_id: Uuid },
}

#[derive(Debug, Deserialize)]
pub struct GetUserQuery {
    pub user_id: Uuid,
}

#[derive(Clone)]
pub struct GetUserQueryHandler {
    user_dao: UserDao,
}

impl GetUserQueryHandler {
    pub fn new(db: SqlConnect) -> Self {
        Self {
            user_dao: UserDao::new(db),
        }
    }

    #[instrument(skip(self))]
    pub async fn execute(
        &self, query: GetUserQuery,
    ) -> Result<users::Model, GetUserError> {
        let user =
            self.user_dao.find_by_id(query.user_id).await.map_err(|_| {
                GetUserError::NotFound {
                    user_id: query.user_id,
                }
            })?;

        Ok(user)
    }
}
