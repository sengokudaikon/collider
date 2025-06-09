use chrono::{DateTime, Utc};
use database_traits::dao::GenericDao;
use serde::{Deserialize, Serialize};
use sql_connection::SqlConnect;
use thiserror::Error;
use tracing::instrument;
use user_dao::UserDao;
use user_models as users;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum UpdateUserError {
    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),
    #[error("DAO error: {0}")]
    Dao(#[from] user_dao::UserDaoError),
    #[error("User not found: {user_id}")]
    NotFound { user_id: Uuid },
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateUserCommand {
    #[serde(skip)]
    pub user_id: Uuid,
    pub name: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UpdateUserResponse {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct UpdateUserResult {
    pub user: UpdateUserResponse,
    pub events: Vec<UserEvent>,
}

#[derive(Debug, Serialize)]
pub struct UserEvent {
    pub event_type: String,
    pub user_id: Uuid,
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
    ) -> Result<UpdateUserResult, UpdateUserError> {
        let existing_user =
            self.user_dao.find_by_id(command.user_id).await.map_err(
                |_| {
                    UpdateUserError::NotFound {
                        user_id: command.user_id,
                    }
                },
            )?;
        let mut user_active: users::ActiveModel =
            existing_user.clone().into();

        let name_updated = command.name.is_some();
        let user_id = command.user_id;

        if let Some(name) = command.name {
            user_active.name = sea_orm::ActiveValue::Set(name);
        }
        let updated_user =
            self.user_dao.update(command.user_id, user_active).await?;

        let mut events = vec![];

        if name_updated {
            events.push(UserEvent {
                event_type: "user_name_updated".to_string(),
                user_id,
            });
        }

        Ok(UpdateUserResult {
            user: UpdateUserResponse {
                id: updated_user.id,
                name: updated_user.name,
                created_at: updated_user.created_at,
            },
            events,
        })
    }
}

#[cfg(test)]
mod tests {
    use test_utils::*;
    use uuid::Uuid;

    use super::*;

    async fn setup_test_db() -> anyhow::Result<(
        test_utils::postgres::TestPostgresContainer,
        UpdateUserHandler,
    )> {
        let container =
            test_utils::postgres::TestPostgresContainer::new().await?;
        let sql_connect = create_sql_connect(&container);
        let handler = UpdateUserHandler::new(sql_connect);
        Ok((container, handler))
    }

    #[tokio::test]
    async fn test_update_user_name_success() {
        let (container, handler) = setup_test_db().await.unwrap();
        let user_id = create_test_user(&container).await.unwrap();

        let command = UpdateUserCommand {
            user_id,
            name: Some("Updated Name".to_string()),
        };
        let result = handler.execute(command).await.unwrap();

        assert_eq!(result.user.id, user_id);
        assert_eq!(result.user.name, "Updated Name");
        assert_eq!(result.events.len(), 1);
        assert_eq!(result.events[0].event_type, "user_name_updated");
        assert_eq!(result.events[0].user_id, user_id);
    }

    #[tokio::test]
    async fn test_update_user_no_changes() {
        let (container, handler) = setup_test_db().await.unwrap();
        let user_id = create_test_user(&container).await.unwrap();

        let command = UpdateUserCommand {
            user_id,
            name: None,
        };
        let result = handler.execute(command).await.unwrap();

        assert_eq!(result.user.id, user_id);
        assert_eq!(result.events.len(), 0);
    }

    #[tokio::test]
    async fn test_update_user_not_found() {
        let (_container, handler) = setup_test_db().await.unwrap();
        let non_existent_user_id = Uuid::now_v7();

        let command = UpdateUserCommand {
            user_id: non_existent_user_id,
            name: Some("New Name".to_string()),
        };
        let result = handler.execute(command).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            UpdateUserError::NotFound { user_id } => {
                assert_eq!(user_id, non_existent_user_id);
            }
            _ => panic!("Expected NotFound error"),
        }
    }
}
