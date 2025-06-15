use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

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
}
