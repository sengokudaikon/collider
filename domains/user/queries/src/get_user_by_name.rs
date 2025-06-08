use serde::{Deserialize, Serialize};
use sql_connection::SqlConnect;
use thiserror::Error;
use tracing::instrument;
use user_dao::{UserDao, UserDaoError};
use user_models as users;

#[derive(Debug, Error)]
pub enum GetUserByNameError {
    #[error("DAO error: {0}")]
    Dao(#[from] UserDaoError),
    #[error("User not found with name: {0}")]
    NotFound(String),
}

#[derive(Debug, Deserialize)]
pub struct GetUserByNameQuery {
    pub username: String,
}

#[derive(Debug, Serialize)]
pub struct GetUserByNameResponse {
    pub id: uuid::Uuid,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<users::Model> for GetUserByNameResponse {
    fn from(user: users::Model) -> Self {
        Self {
            id: user.id,
            name: user.name,
            created_at: user.created_at,
        }
    }
}

#[derive(Clone)]
pub struct GetUserByNameQueryHandler {
    user_dao: UserDao,
}

impl GetUserByNameQueryHandler {
    pub fn new(db: SqlConnect) -> Self {
        Self {
            user_dao: UserDao::new(db),
        }
    }

    #[instrument(skip(self))]
    pub async fn execute(
        &self, query: GetUserByNameQuery,
    ) -> Result<GetUserByNameResponse, GetUserByNameError> {
        let user = self
            .user_dao
            .find_by_name(&query.username)
            .await?
            .ok_or_else(|| {
                GetUserByNameError::NotFound(query.username.clone())
            })?;

        Ok(user.into())
    }
}
