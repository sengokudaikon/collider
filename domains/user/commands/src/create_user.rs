use database_traits::dao::GenericDao;
use serde::{Deserialize, Serialize};
use sql_connection::SqlConnect;
use thiserror::Error;
use tracing::instrument;
use user_dao::UserDao;
use user_models::NewUser;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum CreateUserError {
    #[error("DAO error: {0}")]
    Dao(#[from] user_dao::UserDaoError),
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateUserCommand {
    pub name: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CreateUserResponse {
    pub id: Uuid,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct CreateUserResult {
    pub user: CreateUserResponse,
    pub events: Vec<UserEvent>,
}

#[derive(Debug, Serialize)]
pub struct UserEvent {
    pub event_type: String,
    pub user_id: Uuid,
}

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
    ) -> Result<CreateUserResult, CreateUserError> {
        let new_user = NewUser {
            id: Uuid::now_v7(),
            name: command.name,
        };

        let saved_user = self.user_dao.create(new_user).await?;

        let events = vec![UserEvent {
            event_type: "user_created".to_string(),
            user_id: saved_user.id,
        }];

        Ok(CreateUserResult {
            user: CreateUserResponse {
                id: saved_user.id,
                name: saved_user.name,
                created_at: saved_user.created_at,
            },
            events,
        })
    }
}

#[cfg(test)]
mod tests {
    use test_utils::*;

    use super::*;

    async fn setup_test_db() -> TestPostgresContainer {
        TestPostgresContainer::new().await.unwrap()
    }

    #[tokio::test]
    async fn test_create_user_handler_new() {
        let container = setup_test_db().await;
        let sql_connect = create_sql_connect(&container);
        let _handler = CreateUserHandler::new(sql_connect);
        // Just test construction doesn't panic
    }

    #[tokio::test]
    async fn test_execute_create_user() {
        let container = setup_test_db().await;
        let sql_connect = create_sql_connect(&container);
        let handler = CreateUserHandler::new(sql_connect);

        let command = CreateUserCommand {
            name: "test_user".to_string(),
        };

        let result = handler.execute(command).await.unwrap();

        assert_eq!(result.user.name, "test_user");
        assert!(!result.user.id.is_nil());
        assert_eq!(result.events.len(), 1);
        assert_eq!(result.events[0].event_type, "user_created");
        assert_eq!(result.events[0].user_id, result.user.id);
    }

    #[tokio::test]
    async fn test_execute_create_user_with_empty_name() {
        let container = setup_test_db().await;
        let sql_connect = create_sql_connect(&container);
        let handler = CreateUserHandler::new(sql_connect);

        let command = CreateUserCommand {
            name: "".to_string(),
        };

        let result = handler.execute(command).await.unwrap();
        assert_eq!(result.user.name, "");
    }

    #[tokio::test]
    async fn test_execute_create_user_with_long_name() {
        let container = setup_test_db().await;
        let sql_connect = create_sql_connect(&container);
        let handler = CreateUserHandler::new(sql_connect);

        let long_name = "a".repeat(50);
        let command = CreateUserCommand {
            name: long_name.clone(),
        };

        let result = handler.execute(command).await.unwrap();
        assert_eq!(result.user.name, long_name);
    }

    #[tokio::test]
    async fn test_multiple_users_creation() {
        let container = setup_test_db().await;
        let sql_connect = create_sql_connect(&container);
        let handler = CreateUserHandler::new(sql_connect);

        let command1 = CreateUserCommand {
            name: "user1".to_string(),
        };
        let command2 = CreateUserCommand {
            name: "user2".to_string(),
        };

        let result1 = handler.execute(command1).await.unwrap();
        let result2 = handler.execute(command2).await.unwrap();

        assert_ne!(result1.user.id, result2.user.id);
        assert_eq!(result1.user.name, "user1");
        assert_eq!(result2.user.name, "user2");
    }
}