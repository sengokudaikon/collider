use database_traits::dao::GenericDao;
use sea_orm::DbErr;
use serde::Deserialize;
use sql_connection::SqlConnect;
use thiserror::Error;
use tracing::instrument;
use user_dao::UserDao;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum DeleteUserError {
    #[error("Database error: {0}")]
    Database(#[from] DbErr),
    #[error("DAO error: {0}")]
    Dao(#[from] user_dao::UserDaoError),
    #[error("User not found: {user_id}")]
    NotFound { user_id: Uuid },
}

#[derive(Debug, Deserialize)]
pub struct DeleteUserCommand {
    pub user_id: Uuid,
}

#[derive(Clone)]
pub struct DeleteUserHandler {
    user_dao: UserDao,
}

impl DeleteUserHandler {
    pub fn new(db: SqlConnect) -> Self {
        Self {
            user_dao: UserDao::new(db),
        }
    }

    #[instrument(skip(self))]
    pub async fn execute(
        &self, command: DeleteUserCommand,
    ) -> Result<(), DeleteUserError> {
        let _existing_user =
            self.user_dao.find_by_id(command.user_id).await.map_err(
                |_| {
                    DeleteUserError::NotFound {
                        user_id: command.user_id,
                    }
                },
            )?;

        self.user_dao.delete(command.user_id).await?;
        Ok(())
    }
}
