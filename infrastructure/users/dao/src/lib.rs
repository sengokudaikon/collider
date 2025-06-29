use async_trait::async_trait;
use chrono::Utc;
use dao_utils::{
    pagination::{CursorPagination, PaginationParams, create_param_refs},
    query_helpers::{CursorResult, count_query},
};
use database_traits::dao::GenericDao;
use sql_connection::{PgError, SqlConnect};
use tracing::instrument;
use user_commands::{CreateUserCommand, UpdateUserCommand};
use user_errors::UserError;
use user_models::User;

#[derive(Clone)]
pub struct UserDao {
    db: SqlConnect,
}

impl UserDao {
    pub fn new(db: SqlConnect) -> Self { Self { db } }

    pub fn db(&self) -> &SqlConnect { &self.db }

    #[instrument(skip(self))]
    pub async fn find_by_name(
        &self, name: &str,
    ) -> Result<Option<User>, UserError> {
        let client = self.db.get_read_client().await?;
        let stmt = client
            .prepare("SELECT id, name, created_at FROM users WHERE name = $1")
            .await?;
        let rows = client.query(&stmt, &[&name]).await?;

        let user = rows.first().map(|row| self.map_row(row));

        Ok(user)
    }
}

#[async_trait]
impl GenericDao for UserDao {
    type CreateRequest = CreateUserCommand;
    type Error = UserError;
    type ID = i64;
    type Model = User;
    type Response = User;
    type UpdateRequest = UpdateUserCommand;

    async fn find_by_id(
        &self, id: Self::ID,
    ) -> Result<Self::Response, Self::Error> {
        let client = self.db.get_read_client().await?;
        let stmt =
            client.prepare("SELECT * FROM users WHERE id = $1").await?;
        let rows = client.query(&stmt, &[&id]).await?;

        let user = rows
            .first()
            .map(|row| self.map_row(row))
            .ok_or(UserError::NotFound { user_id: id })?;

        Ok(user)
    }

    async fn all(&self) -> Result<Vec<Self::Response>, Self::Error> {
        let client = self.db.get_read_client().await?;
        let stmt = client
            .prepare("SELECT * FROM users ORDER BY name ASC")
            .await?;
        let rows = client.query(&stmt, &[]).await?;

        let users = rows.iter().map(|row| self.map_row(row)).collect();

        Ok(users)
    }

    async fn create(
        &self, req: Self::CreateRequest,
    ) -> Result<Self::Response, Self::Error> {
        let client = self.db.get_client().await?;
        let created_at = Utc::now();

        let stmt = client
            .prepare(
                "WITH name_check AS (
                     SELECT EXISTS(SELECT 1 FROM users WHERE name = $1) as \
                 name_exists
                 ),
                 inserted AS (
                     INSERT INTO users (name, created_at) 
                     SELECT $1, $2
                     WHERE NOT EXISTS(SELECT 1 FROM name_check WHERE \
                 name_exists = true)
                     RETURNING id, name, created_at
                 )
                 SELECT i.id, i.name, i.created_at, nc.name_exists
                 FROM name_check nc
                 LEFT JOIN inserted i ON nc.name_exists = false",
            )
            .await?;

        let rows = client.query(&stmt, &[&req.name, &created_at]).await?;

        if let Some(row) = rows.first() {
            let name_exists: bool = row.get(3);
            if name_exists {
                return Err(UserError::NameExists);
            }

            let user = User {
                id: row.get(0),
                name: row.get(1),
                created_at: row.get(2),
            };
            Ok(user)
        }
        else {
            Err(UserError::Database(PgError::__private_api_timeout()))
        }
    }

    async fn update(
        &self, id: Self::ID, req: Self::UpdateRequest,
    ) -> Result<Self::Response, Self::Error> {
        let client = self.db.get_client().await?;

        match &req.name {
            Some(new_name) => {
                let stmt = client
                    .prepare(
                        "WITH conflict_check AS (
                             SELECT CASE 
                                 WHEN NOT EXISTS(SELECT 1 FROM users WHERE \
                         id = $2) THEN 'not_found'::text
                                 WHEN EXISTS(SELECT 1 FROM users WHERE name \
                         = $1 AND id != $2) THEN 'name_exists'::text
                                 ELSE 'ok'::text
                             END as status
                         ),
                         updated AS (
                             UPDATE users 
                             SET name = $1 
                             WHERE id = $2 
                             AND (SELECT status FROM conflict_check) = 'ok'
                             RETURNING id, name, created_at
                         )
                         SELECT u.id, u.name, u.created_at, c.status
                         FROM conflict_check c
                         LEFT JOIN updated u ON c.status = 'ok'",
                    )
                    .await?;

                let rows = client.query(&stmt, &[new_name, &id]).await?;

                if let Some(row) = rows.first() {
                    let status: String = row.get(3);
                    match status.as_str() {
                        "not_found" => {
                            Err(UserError::NotFound { user_id: id })
                        }
                        "name_exists" => Err(UserError::NameExists),
                        "ok" => {
                            let user = self.map_row(row);
                            Ok(user)
                        }
                        _ => {
                            Err(UserError::Database(
                                PgError::__private_api_timeout(),
                            ))
                        }
                    }
                }
                else {
                    Err(UserError::Database(PgError::__private_api_timeout()))
                }
            }
            None => self.find_by_id(id).await,
        }
    }

    async fn delete(&self, id: Self::ID) -> Result<(), Self::Error> {
        let client = self.db.get_client().await?;

        let stmt = client
            .prepare("DELETE FROM users WHERE id = $1 RETURNING id")
            .await?;
        let rows = client.execute(&stmt, &[&id]).await?;

        if rows == 0 {
            return Err(UserError::NotFound { user_id: id });
        }

        Ok(())
    }

    async fn count(&self) -> Result<i64, Self::Error> {
        let client = self.db.get_read_client().await?;
        let count = count_query(&client, "users").await?;
        Ok(count)
    }

    fn map_row(&self, row: &tokio_postgres::Row) -> Self::Model {
        User {
            id: row.get(0),
            name: row.get(1),
            created_at: row.get(2),
        }
    }
}

impl UserDao {
    #[instrument(skip_all)]
    pub async fn find_with_pagination(
        &self, limit: Option<u64>, offset: Option<u64>,
    ) -> Result<Vec<User>, UserError> {
        let client = self.db.get_read_client().await?;
        let pagination = PaginationParams::new(limit, offset);
        let (sql, params) = pagination.build_query_parts(
            "SELECT id, name, created_at FROM users",
            "ORDER BY name ASC",
        );

        let stmt = client.prepare(&sql).await?;
        let param_refs = create_param_refs(&params);
        let rows = client.query(&stmt, &param_refs).await?;
        let users = rows.iter().map(|row| self.map_row(row)).collect();

        Ok(users)
    }

    #[instrument(skip_all)]
    pub async fn find_with_cursor(
        &self, cursor: Option<String>, limit: u64,
    ) -> Result<CursorResult<User, String>, UserError> {
        let client = self.db.get_read_client().await?;
        let pagination = CursorPagination::new(cursor.clone(), limit);
        let limit_plus_one = pagination.limit_plus_one();

        let rows = match cursor {
            Some(cursor_name) => {
                let sql = "SELECT id, name, created_at FROM users 
                          WHERE name > $1 
                          ORDER BY name ASC 
                          LIMIT $2";
                let stmt = client.prepare(sql).await?;
                client
                    .query(&stmt, &[&cursor_name, &limit_plus_one])
                    .await?
            }
            None => {
                let sql = "SELECT id, name, created_at FROM users 
                          ORDER BY name ASC 
                          LIMIT $1";
                let stmt = client.prepare(sql).await?;
                client.query(&stmt, &[&limit_plus_one]).await?
            }
        };

        let users: Vec<User> = rows
            .iter()
            .take(pagination.limit as usize)
            .map(|row| self.map_row(row))
            .collect();

        let next_cursor = if rows.len() > pagination.limit as usize {
            users.last().map(|u| u.name.clone())
        }
        else {
            None
        };

        Ok(CursorResult::new(users, next_cursor))
    }
}

#[cfg(test)]
mod tests {
    use database_traits::dao::GenericDao;
    use test_utils::*;
    use user_commands::{CreateUserCommand, UpdateUserCommand};

    use crate::{UserDao, UserError};

    async fn setup_test_db() -> TestPostgresContainer {
        TestPostgresContainer::new().await.unwrap()
    }

    fn create_test_user(name: &str) -> CreateUserCommand {
        CreateUserCommand {
            name: name.to_string(),
        }
    }

    #[tokio::test]
    async fn test_user_dao_new() {
        let container = setup_test_db().await;
        let sql_connect = create_sql_connect(&container);
        let _dao = UserDao::new(sql_connect);
        // Test passed if we reach this point without panicking
    }

    #[tokio::test]
    async fn test_create_user() {
        let container = setup_test_db().await;
        let sql_connect = create_sql_connect(&container);
        let dao = UserDao::new(sql_connect);

        let user_model = create_test_user("test_user");
        let created_user = dao.create(user_model).await.unwrap();

        assert_eq!(created_user.name, "test_user");
        assert!(created_user.id > 0);
    }

    #[tokio::test]
    async fn test_find_by_id() {
        let container = setup_test_db().await;
        let sql_connect = create_sql_connect(&container);
        let dao = UserDao::new(sql_connect);

        let user_model = create_test_user("find_by_id_user");
        let created_user = dao.create(user_model).await.unwrap();

        let found_user = dao.find_by_id(created_user.id).await.unwrap();
        assert_eq!(found_user.name, "find_by_id_user");
        assert_eq!(found_user.id, created_user.id);
    }

    #[tokio::test]
    async fn test_find_by_id_not_found() {
        let container = setup_test_db().await;
        let sql_connect = create_sql_connect(&container);
        let dao = UserDao::new(sql_connect);
        let id = 999999;
        let result = dao.find_by_id(id).await;
        assert!(
            matches!(result, Err(UserError::NotFound { user_id }) if user_id == id)
        );
    }

    #[tokio::test]
    async fn test_find_by_name() {
        let container = setup_test_db().await;
        let sql_connect = create_sql_connect(&container);
        let dao = UserDao::new(sql_connect);

        let user_model = create_test_user("name_search_user");
        let created_user = dao.create(user_model).await.unwrap();

        let found_user = dao.find_by_name("name_search_user").await.unwrap();
        assert!(found_user.is_some());
        let found_user = found_user.unwrap();
        assert_eq!(found_user.name, "name_search_user");
        assert_eq!(found_user.id, created_user.id);
    }

    #[tokio::test]
    async fn test_find_by_name_not_found() {
        let container = setup_test_db().await;
        let sql_connect = create_sql_connect(&container);
        let dao = UserDao::new(sql_connect);

        let found_user = dao.find_by_name("nonexistent_user").await.unwrap();
        assert!(found_user.is_none());
    }

    #[tokio::test]
    async fn test_all_users() {
        let container = setup_test_db().await;
        let sql_connect = create_sql_connect(&container);
        let dao = UserDao::new(sql_connect);

        let user1 = create_test_user("user_1");
        let user2 = create_test_user("user_2");
        let user3 = create_test_user("user_3");

        dao.create(user1).await.unwrap();
        dao.create(user2).await.unwrap();
        dao.create(user3).await.unwrap();

        let all_users = dao.all().await.unwrap();
        assert_eq!(all_users.len(), 3); // 3 created users

        // Check they are ordered by name
        let names: Vec<&String> = all_users.iter().map(|u| &u.name).collect();
        assert_eq!(names, vec!["user_1", "user_2", "user_3"]);
    }

    #[tokio::test]
    async fn test_update_user() {
        let container = setup_test_db().await;
        let sql_connect = create_sql_connect(&container);
        let dao = UserDao::new(sql_connect);

        let user_model = create_test_user("original_name");
        let created_user = dao.create(user_model).await.unwrap();

        let update_model = UpdateUserCommand {
            user_id: created_user.id,
            name: Some("updated_name".to_string()),
        };

        let updated_user =
            dao.update(created_user.id, update_model).await.unwrap();
        assert_eq!(updated_user.name, "updated_name");
        assert_eq!(updated_user.id, created_user.id);
    }

    #[tokio::test]
    async fn test_update_nonexistent_user() {
        let container = setup_test_db().await;
        let sql_connect = create_sql_connect(&container);
        let dao = UserDao::new(sql_connect);
        let id = 999999;
        let update_model = UpdateUserCommand {
            user_id: id,
            name: Some("updated_name".to_string()),
        };

        let result = dao.update(id, update_model).await;
        assert!(
            matches!(result, Err(UserError::NotFound { user_id }) if user_id == id)
        );
    }

    #[tokio::test]
    async fn test_delete_user() {
        let container = setup_test_db().await;
        let sql_connect = create_sql_connect(&container);
        let dao = UserDao::new(sql_connect);

        let user_model = create_test_user("delete_me");
        let created_user = dao.create(user_model).await.unwrap();

        dao.delete(created_user.id).await.unwrap();

        let result = dao.find_by_id(created_user.id).await;
        assert!(
            matches!(result, Err(UserError::NotFound { user_id }) if user_id == created_user.id)
        );
    }

    #[tokio::test]
    async fn test_find_with_pagination() {
        let container = setup_test_db().await;
        let sql_connect = create_sql_connect(&container);
        let dao = UserDao::new(sql_connect);

        for i in 1..=5 {
            let user_model = create_test_user(&format!("user_{i:02}"));
            dao.create(user_model).await.unwrap();
        }

        let limited = dao.find_with_pagination(Some(3), None).await.unwrap();
        assert_eq!(limited.len(), 3);

        let offset = dao.find_with_pagination(None, Some(2)).await.unwrap();
        assert_eq!(offset.len(), 3); // 5 total users - 2 offset = 3 remaining

        let paginated =
            dao.find_with_pagination(Some(2), Some(1)).await.unwrap();
        assert_eq!(paginated.len(), 2);
        assert_eq!(paginated[0].name, "user_02"); // user_01 is at position 0, user_02 is at position 1

        let all = dao.find_with_pagination(None, None).await.unwrap();
        assert_eq!(all.len(), 5); // 5 created users
    }

    #[tokio::test]
    async fn test_find_with_cursor_pagination() {
        let container = setup_test_db().await;
        let sql_connect = create_sql_connect(&container);
        let dao = UserDao::new(sql_connect);

        for i in 1..=5 {
            let user_model = create_test_user(&format!("user_{i:02}"));
            dao.create(user_model).await.unwrap();
        }

        let first_page_result = dao.find_with_cursor(None, 2).await.unwrap();
        assert_eq!(first_page_result.items.len(), 2);
        assert_eq!(first_page_result.items[0].name, "user_01");
        assert_eq!(first_page_result.items[1].name, "user_02");
        assert!(first_page_result.next_cursor.is_some());

        let second_page_result = dao
            .find_with_cursor(first_page_result.next_cursor, 2)
            .await
            .unwrap();
        assert_eq!(second_page_result.items.len(), 2);
        assert_eq!(second_page_result.items[0].name, "user_03");
        assert_eq!(second_page_result.items[1].name, "user_04");
        assert!(second_page_result.next_cursor.is_some());

        let final_page_result = dao
            .find_with_cursor(second_page_result.next_cursor, 2)
            .await
            .unwrap();
        assert_eq!(final_page_result.items.len(), 1); // Only one user left
        assert_eq!(final_page_result.items[0].name, "user_05");
        assert!(final_page_result.next_cursor.is_none());
    }

    #[tokio::test]
    async fn test_count() {
        let container = setup_test_db().await;
        let sql_connect = create_sql_connect(&container);
        let dao = UserDao::new(sql_connect);

        // Initially should have 0 users
        let initial_count = dao.count().await.unwrap();
        assert_eq!(initial_count, 0);

        // Create some users
        for i in 1..=3 {
            let user_model = create_test_user(&format!("count_user_{i}"));
            dao.create(user_model).await.unwrap();
        }

        // Should now have 3 users (3 created)
        let final_count = dao.count().await.unwrap();
        assert_eq!(final_count, 3);
    }

    #[tokio::test]
    async fn test_map_row() {
        let container = setup_test_db().await;
        let sql_connect = create_sql_connect(&container);
        let dao = UserDao::new(sql_connect);

        let user_model = create_test_user("map_row_test");
        let created_user = dao.create(user_model).await.unwrap();

        // Test that map_row works correctly by finding the user
        let found_user = dao.find_by_id(created_user.id).await.unwrap();

        assert_eq!(found_user.id, created_user.id);
        assert_eq!(found_user.name, "map_row_test");
        assert_eq!(
            found_user.created_at.timestamp(),
            created_user.created_at.timestamp()
        );
    }
}
