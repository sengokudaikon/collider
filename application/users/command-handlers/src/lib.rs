use database_traits::dao::GenericDao;
use sql_connection::SqlConnect;
use tracing::instrument;
use user_commands::{
    CreateUserCommand, DeleteUserCommand, UpdateUserCommand,
};
use user_dao::UserDao;
use user_errors::UserError;
use user_responses::UserResponse;

#[derive(Clone)]
pub struct CreateUserHandler {
    user_dao: UserDao,
}

impl CreateUserHandler {
    pub fn new(db: SqlConnect) -> Self {
        Self {
            user_dao: UserDao::new(db),
        }
    }

    #[instrument(skip(self))]
    pub async fn execute(
        &self, command: CreateUserCommand,
    ) -> Result<UserResponse, UserError> {
        let saved_user = self.user_dao.create(command).await?;

        Ok(UserResponse {
            id: saved_user.id,
            name: saved_user.name,
            created_at: saved_user.created_at,
        })
    }
}

#[derive(Clone)]
pub struct UpdateUserHandler {
    user_dao: UserDao,
}

impl UpdateUserHandler {
    pub fn new(db: SqlConnect) -> Self {
        Self {
            user_dao: UserDao::new(db),
        }
    }

    #[instrument(skip(self))]
    pub async fn execute(
        &self, command: UpdateUserCommand,
    ) -> Result<UserResponse, UserError> {
        let updated_user =
            self.user_dao.update(command.user_id, command).await?;

        Ok(UserResponse {
            id: updated_user.id,
            name: updated_user.name,
            created_at: updated_user.created_at,
        })
    }
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
    ) -> Result<(), UserError> {
        let _existing_user =
            self.user_dao.find_by_id(command.user_id).await.map_err(
                |_| {
                    UserError::NotFound {
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

    use super::*;

    async fn setup_test_handlers() -> anyhow::Result<(
        test_utils::TestPostgresContainer,
        CreateUserHandler,
        UpdateUserHandler,
        DeleteUserHandler,
    )> {
        let container = test_utils::TestPostgresContainer::new().await?;
        let sql_connect = create_sql_connect(&container);

        let create_handler = CreateUserHandler::new(sql_connect.clone());
        let update_handler = UpdateUserHandler::new(sql_connect.clone());
        let delete_handler = DeleteUserHandler::new(sql_connect);

        Ok((container, create_handler, update_handler, delete_handler))
    }

    #[tokio::test]
    async fn test_create_user_handler() {
        let (_container, create_handler, ..) =
            setup_test_handlers().await.unwrap();

        let command = CreateUserCommand {
            name: "test_user".to_string(),
        };

        let result = create_handler.execute(command).await.unwrap();

        assert_eq!(result.name, "test_user");
        assert!(!result.id.is_nil());
    }
}
