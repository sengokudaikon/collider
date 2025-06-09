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

#[cfg(test)]
mod tests {
    use test_utils::*;
    use uuid::Uuid;

    use super::*;

    async fn setup_test_db() -> anyhow::Result<(
        test_utils::postgres::TestPostgresContainer,
        DeleteUserHandler,
    )> {
        let container =
            test_utils::postgres::TestPostgresContainer::new().await?;
        let sql_connect = create_sql_connect(&container);
        let handler = DeleteUserHandler::new(sql_connect);
        Ok((container, handler))
    }

    #[tokio::test]
    async fn test_delete_user_success() {
        let (container, handler) = setup_test_db().await.unwrap();
        let user_id = create_test_user(&container).await.unwrap();

        let command = DeleteUserCommand { user_id };
        let result = handler.execute(command).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_delete_user_not_found() {
        let (_container, handler) = setup_test_db().await.unwrap();
        let non_existent_user_id = Uuid::now_v7();

        let command = DeleteUserCommand {
            user_id: non_existent_user_id,
        };
        let result = handler.execute(command).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            DeleteUserError::NotFound { user_id } => {
                assert_eq!(user_id, non_existent_user_id);
            }
            _ => panic!("Expected NotFound error"),
        }
    }
}
