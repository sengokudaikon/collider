use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateUserCommand {
    #[serde(skip)]
    pub user_id: Uuid,
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateUserCommand {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct DeleteUserCommand {
    pub user_id: Uuid,
}
