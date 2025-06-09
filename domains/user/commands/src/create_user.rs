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
pub enum CreateUserError {
    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),
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
        let user_active = users::ActiveModel {
            id: sea_orm::ActiveValue::Set(Uuid::now_v7()),
            name: sea_orm::ActiveValue::Set(command.name),
            created_at: sea_orm::ActiveValue::Set(chrono::Utc::now()),
        };

        let saved_user = self.user_dao.create(user_active).await?;

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
