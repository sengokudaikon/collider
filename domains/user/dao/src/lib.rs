use async_trait::async_trait;
use database_traits::{
    dao::GenericDao,
    transaction::{GetDatabaseTransaction, TransactionOps},
};
use sea_orm::{sea_query::IntoCondition, *};
use sql_connection::SqlConnect;
use thiserror::Error;
use tracing::instrument;
use user_models as users;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum UserDaoError {
    #[error("Database error: {0}")]
    Database(#[from] DbErr),
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
    ) -> Result<Option<users::Model>, UserDaoError> {
        let ctx = self.db.get_transaction().await?;
        let user = users::Entity::find()
            .filter(users::Column::Name.eq(name))
            .one(&ctx)
            .await?;
        ctx.submit().await?;
        Ok(user)
    }
}

#[async_trait]
impl GenericDao for UserDao {
    type ActiveModel = users::ActiveModel;
    type CreateRequest = users::ActiveModel;
    type Entity = users::Entity;
    type Error = UserDaoError;
    type ID = Uuid;
    type Model = users::Model;
    type Response = users::Model;
    type UpdateRequest = users::ActiveModel;

    #[instrument(skip_all)]
    async fn query_one(
        &self,
        condition: impl Into<Option<Condition>> + Send + Sync + 'static,
        db: &impl ConnectionTrait,
    ) -> Result<Self::Response, Self::Error> {
        let condition = condition.into().unwrap_or_else(Condition::all);
        let model = users::Entity::find()
            .filter(condition)
            .one(db)
            .await?
            .ok_or(UserDaoError::NotFound)?;

        Ok(model)
    }

    #[instrument(skip_all)]
    async fn query_all(
        &self,
        condition: impl Into<Option<Condition>> + Send + Sync + 'static,
        db: &impl ConnectionTrait,
    ) -> Result<Vec<Self::Response>, Self::Error> {
        let condition = condition.into().unwrap_or_else(Condition::all);
        let models = users::Entity::find()
            .filter(condition)
            .order_by_asc(users::Column::Name)
            .all(db)
            .await?;

        Ok(models)
    }

    async fn find_by_id(
        &self, id: Self::ID,
    ) -> Result<Self::Response, Self::Error> {
        let ctx = self.db.get_transaction().await?;
        let condition = users::Column::Id.eq(id).into_condition();
        let result = self.query_one(condition, &ctx).await;
        ctx.submit().await?;
        result
    }

    async fn all(&self) -> Result<Vec<Self::Response>, Self::Error> {
        let ctx = self.db.get_transaction().await?;
        let result = self.query_all(Condition::all(), &ctx).await;
        ctx.submit().await?;
        result
    }

    async fn create(
        &self, req: Self::CreateRequest,
    ) -> Result<Self::Response, Self::Error> {
        let ctx = self.db.get_transaction().await?;
        let result = req.insert(&ctx).await?;
        ctx.submit().await?;
        Ok(result)
    }

    async fn update(
        &self, id: Self::ID, req: Self::UpdateRequest,
    ) -> Result<Self::Response, Self::Error> {
        let ctx = self.db.get_transaction().await?;

        let _existing = users::Entity::find_by_id(id)
            .one(&ctx)
            .await?
            .ok_or(UserDaoError::NotFound)?;

        let result = req.update(&ctx).await?;
        ctx.submit().await?;
        Ok(result)
    }

    async fn delete(&self, id: Self::ID) -> Result<(), Self::Error> {
        let ctx = self.db.get_transaction().await?;
        users::Entity::delete_by_id(id).exec(&ctx).await?;
        ctx.submit().await?;
        Ok(())
    }
}

impl UserDao {
    #[instrument(skip_all)]
    pub async fn find_with_pagination(
        &self, limit: Option<u64>, offset: Option<u64>,
    ) -> Result<Vec<users::Model>, UserDaoError> {
        let ctx = self.db.get_transaction().await?;

        let mut query =
            users::Entity::find().order_by_asc(users::Column::Name);

        if let Some(offset) = offset {
            query = query.offset(offset);
        }

        if let Some(limit) = limit {
            query = query.limit(limit);
        }

        let models = query.all(&ctx).await?;
        ctx.submit().await?;

        Ok(models)
    }
}

#[cfg(test)]
mod tests {
    use database_traits::dao::GenericDao;
    use sea_orm::{ActiveValue, sqlx::types::chrono};
    use test_utils::*;
    use user_models as users;
    use uuid::Uuid;

    use crate::{UserDao, UserDaoError};

    async fn setup_test_db() -> TestPostgresContainer {
        TestPostgresContainer::new().await.unwrap()
    }

    fn create_test_user(name: &str) -> users::ActiveModel {
        users::ActiveModel {
            id: ActiveValue::Set(Uuid::now_v7()),
            name: ActiveValue::Set(name.to_string()),
            created_at: ActiveValue::Set(chrono::Utc::now()),
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

        let update_model = users::ActiveModel {
            id: ActiveValue::Set(created_user.id),
            name: ActiveValue::Set("updated_name".to_string()),
            created_at: ActiveValue::NotSet,
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

        let update_model = users::ActiveModel {
            id: ActiveValue::Set(Uuid::now_v7()),
            name: ActiveValue::Set("updated_name".to_string()),
            created_at: ActiveValue::NotSet,
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
