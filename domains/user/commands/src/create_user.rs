use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

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
}


