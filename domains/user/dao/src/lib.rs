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
