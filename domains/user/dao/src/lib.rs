use async_trait::async_trait;
use database_traits::dao::GenericDao;
use sql_connection::SqlConnect;
use thiserror::Error;
use tokio_postgres::Error as PgError;
use tracing::instrument;
use user_models::{User, NewUser, UpdateUser};
use uuid::Uuid;
use chrono::Utc;

#[derive(Debug, Error)]
pub enum UserDaoError {
    #[error("Database error: {0}")]
    Database(#[from] PgError),
    #[error("Connection error: {0}")]
    Connection(#[from] deadpool_postgres::PoolError),
    #[error("User not found")]
    NotFound,
    #[error("Name already exists")]
    NameExists,
}

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
    ) -> Result<Option<User>, UserDaoError> {
        let client = self.db.get_client().await?;
        let stmt = client.prepare("SELECT id, name, created_at FROM users WHERE name = $1").await?;
        let rows = client.query(&stmt, &[&name]).await?;
        
        let user = if let Some(row) = rows.first() {
            Some(User {
                id: row.get(0),
                name: row.get(1),
                created_at: row.get(2),
            })
        } else {
            None
        };
        
        Ok(user)
    }
}

#[async_trait]
impl GenericDao for UserDao {
    type Model = User;
    type Response = User;
    type CreateRequest = NewUser;
    type UpdateRequest = UpdateUser;
    type Error = UserDaoError;
    type ID = Uuid;


    async fn find_by_id(
        &self, id: Self::ID,
    ) -> Result<Self::Response, Self::Error> {
        let client = self.db.get_client().await?;
        let stmt = client.prepare("SELECT id, name, created_at FROM users WHERE id = $1").await?;
        let rows = client.query(&stmt, &[&id]).await?;
        
        let user = rows.first()
            .map(|row| User {
                id: row.get(0),
                name: row.get(1),
                created_at: row.get(2),
            })
            .ok_or(UserDaoError::NotFound)?;
        
        Ok(user)
    }

    async fn all(&self) -> Result<Vec<Self::Response>, Self::Error> {
        let client = self.db.get_client().await?;
        let stmt = client.prepare("SELECT id, name, created_at FROM users ORDER BY name ASC").await?;
        let rows = client.query(&stmt, &[]).await?;
        
        let users = rows.iter()
            .map(|row| User {
                id: row.get(0),
                name: row.get(1),
                created_at: row.get(2),
            })
            .collect();
        
        Ok(users)
    }

    async fn create(
        &self, req: Self::CreateRequest,
    ) -> Result<Self::Response, Self::Error> {
        let client = self.db.get_client().await?;
        let created_at = Utc::now();
        let stmt = client.prepare("INSERT INTO users (id, name, created_at) VALUES ($1, $2, $3) RETURNING id, name, created_at").await?;
        let rows = client.query(&stmt, &[&req.id, &req.name, &created_at]).await?;
        
        let user = rows.first()
            .map(|row| User {
                id: row.get(0),
                name: row.get(1),
                created_at: row.get(2),
            })
            .ok_or_else(|| UserDaoError::Database(PgError::__private_api_timeout()))?;
        
        Ok(user)
    }

    async fn update(
        &self, id: Self::ID, req: Self::UpdateRequest,
    ) -> Result<Self::Response, Self::Error> {
        let client = self.db.get_client().await?;

        // Check if user exists first
        let check_stmt = client.prepare("SELECT id FROM users WHERE id = $1").await?;
        let check_rows = client.query(&check_stmt, &[&id]).await?;
        if check_rows.is_empty() {
            return Err(UserDaoError::NotFound);
        }

        // Update the user
        if let Some(name) = req.name {
            let stmt = client.prepare("UPDATE users SET name = $1 WHERE id = $2 RETURNING id, name, created_at").await?;
            let rows = client.query(&stmt, &[&name, &id]).await?;
            
            let user = rows.first()
                .map(|row| User {
                    id: row.get(0),
                    name: row.get(1),
                    created_at: row.get(2),
                })
                .ok_or(UserDaoError::NotFound)?;
            
            Ok(user)
        } else {
            // No update needed, just return the existing user
            let stmt = client.prepare("SELECT id, name, created_at FROM users WHERE id = $1").await?;
            let rows = client.query(&stmt, &[&id]).await?;
            
            let user = rows.first()
                .map(|row| User {
                    id: row.get(0),
                    name: row.get(1),
                    created_at: row.get(2),
                })
                .ok_or(UserDaoError::NotFound)?;
            
            Ok(user)
        }
    }

    async fn delete(&self, id: Self::ID) -> Result<(), Self::Error> {
        let client = self.db.get_client().await?;
        let stmt = client.prepare("DELETE FROM users WHERE id = $1").await?;
        let affected = client.execute(&stmt, &[&id]).await?;
        
        if affected == 0 {
            return Err(UserDaoError::NotFound);
        }
        
        Ok(())
    }
}

impl UserDao {
    #[instrument(skip_all)]
    pub async fn find_with_pagination(
        &self, limit: Option<u64>, offset: Option<u64>,
    ) -> Result<Vec<User>, UserDaoError> {
        let client = self.db.get_client().await?;

        let (query, limit_i64, offset_i64) = match (limit, offset) {
            (Some(l), Some(o)) => {
                ("SELECT id, name, created_at FROM users ORDER BY name ASC LIMIT $1 OFFSET $2".to_string(), 
                 Some(l as i64), Some(o as i64))
            },
            (Some(l), None) => {
                ("SELECT id, name, created_at FROM users ORDER BY name ASC LIMIT $1".to_string(), 
                 Some(l as i64), None)
            },
            (None, Some(o)) => {
                ("SELECT id, name, created_at FROM users ORDER BY name ASC OFFSET $1".to_string(), 
                 None, Some(o as i64))
            },
            (None, None) => {
                ("SELECT id, name, created_at FROM users ORDER BY name ASC".to_string(), 
                 None, None)
            },
        };

        let stmt = client.prepare(&query).await?;
        let rows = match (limit_i64, offset_i64) {
            (Some(l), Some(o)) => client.query(&stmt, &[&l, &o]).await?,
            (Some(l), None) => client.query(&stmt, &[&l]).await?,
            (None, Some(o)) => client.query(&stmt, &[&o]).await?,
            (None, None) => client.query(&stmt, &[]).await?,
        };
        
        let users = rows.iter()
            .map(|row| User {
                id: row.get(0),
                name: row.get(1),
                created_at: row.get(2),
            })
            .collect();
        
        Ok(users)
    }
}

#[cfg(test)]
mod tests {
    use database_traits::dao::GenericDao;
    use chrono::Utc;
    use test_utils::*;
    use user_models::{User, NewUser, UpdateUser};
    use uuid::Uuid;

    use crate::{UserDao, UserDaoError};

    async fn setup_test_db() -> TestPostgresContainer {
        TestPostgresContainer::new().await.unwrap()
    }

    fn create_test_user(name: &str) -> NewUser {
        NewUser {
            id: Uuid::now_v7(),
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
        assert!(!created_user.id.is_nil());
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

        let result = dao.find_by_id(Uuid::now_v7()).await;
        assert!(matches!(result, Err(UserDaoError::NotFound)));
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

        // Create multiple users
        let user1 = create_test_user("user_1");
        let user2 = create_test_user("user_2");
        let user3 = create_test_user("user_3");

        dao.create(user1).await.unwrap();
        dao.create(user2).await.unwrap();
        dao.create(user3).await.unwrap();

        let all_users = dao.all().await.unwrap();
        assert_eq!(all_users.len(), 3);

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

        let update_model = UpdateUser {
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

        let update_model = UpdateUser {
            name: Some("updated_name".to_string()),
        };

        let result = dao.update(Uuid::now_v7(), update_model).await;
        assert!(matches!(result, Err(UserDaoError::NotFound)));
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
        assert!(matches!(result, Err(UserDaoError::NotFound)));
    }

    #[tokio::test]
    async fn test_find_with_pagination() {
        let container = setup_test_db().await;
        let sql_connect = create_sql_connect(&container);
        let dao = UserDao::new(sql_connect);

        // Create 5 users
        for i in 1..=5 {
            let user_model = create_test_user(&format!("user_{:02}", i));
            dao.create(user_model).await.unwrap();
        }

        // Test limit only
        let limited = dao.find_with_pagination(Some(3), None).await.unwrap();
        assert_eq!(limited.len(), 3);

        // Test offset only
        let offset = dao.find_with_pagination(None, Some(2)).await.unwrap();
        assert_eq!(offset.len(), 3); // Should return 3 users (5 total - 2 offset)

        // Test limit and offset
        let paginated =
            dao.find_with_pagination(Some(2), Some(1)).await.unwrap();
        assert_eq!(paginated.len(), 2);
        assert_eq!(paginated[0].name, "user_02"); // Second user alphabetically

        // Test no limit or offset
        let all = dao.find_with_pagination(None, None).await.unwrap();
        assert_eq!(all.len(), 5);
    }
}
