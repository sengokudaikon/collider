use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct DeleteUserCommand {
    pub user_id: Uuid,
}
