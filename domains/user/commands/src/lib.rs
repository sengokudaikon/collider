use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateUserCommand {
    #[serde(skip)]
    pub user_id: Uuid,
    pub name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateUserCommand {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DeleteUserCommand {
    pub user_id: Uuid,
}
