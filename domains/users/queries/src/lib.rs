use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct ListUsersQuery {
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}
#[derive(Debug, Deserialize)]
pub struct GetUserQuery {
    pub user_id: i64,
}
#[derive(Debug, Deserialize)]
pub struct GetUserByNameQuery {
    pub name: String,
}
