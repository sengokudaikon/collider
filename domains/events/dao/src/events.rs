use async_trait::async_trait;
use database_traits::{
    dao::GenericDao,
    transaction::{GetDatabaseTransaction, TransactionOps},
};
use events_models::{EventActiveModel, EventColumn, EventEntity, EventModel};
use sea_orm::{sea_query::IntoCondition, *};
use sql_connection::SqlConnect;
use thiserror::Error;
use tracing::instrument;
use uuid::Uuid;

use crate::events;

#[derive(Debug, Error)]
pub enum EventDaoError {
    #[error("Database error: {0}")]
    Database(#[from] DbErr),
    #[error("Event not found")]
    NotFound,
}

#[derive(Clone)]
pub struct EventDao {
    db: SqlConnect,
}

impl EventDao {
    pub fn new(db: SqlConnect) -> Self { Self { db } }
}

#[async_trait]
impl GenericDao for EventDao {
    type ActiveModel = EventActiveModel;
    type CreateRequest = EventActiveModel;
    type Entity = EventEntity;
    type Error = EventDaoError;
    type ID = Uuid;
    type Model = EventModel;
    type Response = EventModel;
    type UpdateRequest = EventActiveModel;

    #[instrument(skip_all)]
    async fn query_one(
        &self,
        condition: impl Into<Option<Condition>> + Send + Sync + 'static,
        db: &impl ConnectionTrait,
    ) -> Result<Self::Response, Self::Error> {
        let condition = condition.into().unwrap_or_else(Condition::all);
        let model = {
            let query = EventEntity::find().filter(condition);
            let model = query.one(db).await?;
            model.ok_or(EventDaoError::NotFound)?
        };

        Ok(model)
    }

    #[instrument(skip_all)]
    async fn query_all(
        &self,
        condition: impl Into<Option<Condition>> + Send + Sync + 'static,
        db: &impl ConnectionTrait,
    ) -> Result<Vec<Self::Response>, Self::Error> {
        let condition = condition.into().unwrap_or_else(Condition::all);
        let models = EventEntity::find()
            .filter(condition)
            .order_by_desc(EventColumn::Timestamp)
            .all(db)
            .await?;
        Ok(models)
    }

    async fn find_by_id(
        &self, id: Self::ID,
    ) -> Result<Self::Response, Self::Error> {
        let ctx = self.db.get_transaction().await?;
        let condition = EventColumn::Id.eq(id).into_condition();
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

        let _existing = events::EventEntity::find_by_id(id)
            .one(&ctx)
            .await?
            .ok_or(EventDaoError::NotFound)?;

        let updated_event = req.update(&ctx).await?;
        ctx.submit().await?;
        Ok(updated_event)
    }

    async fn delete(&self, id: Self::ID) -> Result<(), Self::Error> {
        let ctx = self.db.get_transaction().await?;

        // Check if event exists first
        let _event = EventEntity::find_by_id(id)
            .one(&ctx)
            .await?
            .ok_or(EventDaoError::NotFound)?;

        EventEntity::delete_by_id(id).exec(&ctx).await?;
        ctx.submit().await?;
        Ok(())
    }
}

impl EventDao {
    #[instrument(skip_all)]
    pub async fn find_with_filters(
        &self, user_id: Option<Uuid>, event_type_id: Option<i32>,
        limit: Option<u64>, offset: Option<u64>,
    ) -> Result<Vec<EventModel>, EventDaoError> {
        let ctx = self.db.get_transaction().await?;

        let mut query = EventEntity::find();

        if let Some(user_id) = user_id {
            query = query.filter(EventColumn::UserId.eq(user_id));
        }

        if let Some(event_type_id) = event_type_id {
            query = query.filter(EventColumn::EventTypeId.eq(event_type_id));
        }

        query = query.order_by_desc(EventColumn::Timestamp);

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

    #[instrument(skip_all)]
    pub async fn delete_before_timestamp(
        &self, before: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, EventDaoError> {
        let ctx = self.db.get_transaction().await?;

        let delete_result = EventEntity::delete_many()
            .filter(EventColumn::Timestamp.lt(before))
            .exec(&ctx)
            .await?;

        ctx.submit().await?;
        Ok(delete_result.rows_affected)
    }

    #[instrument(skip_all)]
    pub async fn find_by_user_id(
        &self, user_id: Uuid, limit: Option<u64>,
    ) -> Result<Vec<EventModel>, EventDaoError> {
        let ctx = self.db.get_transaction().await?;

        let mut query = EventEntity::find()
            .filter(EventColumn::UserId.eq(user_id))
            .order_by_desc(EventColumn::Timestamp);

        if let Some(limit) = limit {
            query = query.limit(limit);
        }

        let models = query.all(&ctx).await?;
        ctx.submit().await?;

        Ok(models)
    }
}
