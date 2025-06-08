use database_traits::transaction::{GetDatabaseTransaction, TransactionOps};
use events_models::{
    CreateEventTypeRequest, EventTypeActiveModel, EventTypeColumn,
    EventTypeEntity, EventTypeResponse, UpdateEventTypeRequest,
};
use sea_orm::{sea_query::IntoCondition, *};
use sql_connection::SqlConnect;
use thiserror::Error;
use tracing::instrument;

#[derive(Debug, Error)]
pub enum EventTypeDaoError {
    #[error("Database error: {0}")]
    Database(#[from] DbErr),
    #[error("Event type not found")]
    NotFound,
    #[error("Event type with this name already exists")]
    AlreadyExists,
}

#[derive(Clone)]
pub struct EventTypeDao {
    db: SqlConnect,
}

impl EventTypeDao {
    pub fn new(db: SqlConnect) -> Self { Self { db } }
}

impl EventTypeDao {
    #[instrument(skip_all)]
    async fn query_one(
        &self,
        condition: impl Into<Option<Condition>> + Send + Sync + 'static,
        db: &impl ConnectionTrait,
    ) -> Result<EventTypeResponse, EventTypeDaoError> {
        let condition = condition.into().unwrap_or_else(Condition::all);
        let model = {
            let query = EventTypeEntity::find().filter(condition);
            let model = query.one(db).await?;
            model.ok_or(EventTypeDaoError::NotFound)?
        };

        Ok(model.into())
    }

    #[instrument(skip_all)]
    async fn query_all(
        &self,
        condition: impl Into<Option<Condition>> + Send + Sync + 'static,
        db: &impl ConnectionTrait,
    ) -> Result<Vec<EventTypeResponse>, EventTypeDaoError> {
        let condition = condition.into().unwrap_or_else(Condition::all);
        let models = EventTypeEntity::find()
            .filter(condition)
            .order_by_asc(EventTypeColumn::Name)
            .all(db)
            .await?;
        Ok(models.into_iter().map(Into::into).collect())
    }

    pub async fn find_by_id(
        &self, id: i32,
    ) -> Result<EventTypeResponse, EventTypeDaoError> {
        let ctx = self.db.get_transaction().await?;
        let condition = EventTypeColumn::Id.eq(id).into_condition();
        let result = self.query_one(condition, &ctx).await;
        ctx.submit().await?;
        result
    }

    pub async fn all(
        &self,
    ) -> Result<Vec<EventTypeResponse>, EventTypeDaoError> {
        let ctx = self.db.get_transaction().await?;
        let result = self.query_all(Condition::all(), &ctx).await;
        ctx.submit().await?;
        result
    }

    pub async fn create(
        &self, req: CreateEventTypeRequest,
    ) -> Result<EventTypeResponse, EventTypeDaoError> {
        let ctx = self.db.get_transaction().await?;

        if EventTypeEntity::find()
            .filter(EventTypeColumn::Name.eq(&req.name))
            .one(&ctx)
            .await?
            .is_some()
        {
            return Err(EventTypeDaoError::AlreadyExists);
        }

        let event_type_model = EventTypeActiveModel {
            id: NotSet,
            name: Set(req.name),
        };

        let result = event_type_model.insert(&ctx).await?;
        ctx.submit().await?;
        Ok(result.into())
    }

    pub async fn update(
        &self, id: i32, req: UpdateEventTypeRequest,
    ) -> Result<EventTypeResponse, EventTypeDaoError> {
        let ctx = self.db.get_transaction().await?;

        let event_type = EventTypeEntity::find_by_id(id)
            .one(&ctx)
            .await?
            .ok_or(EventTypeDaoError::NotFound)?;

        let mut event_type_active: EventTypeActiveModel = event_type.into();

        if let Some(name) = req.name {
            if let Some(_existing) = EventTypeEntity::find()
                .filter(
                    Condition::all()
                        .add(EventTypeColumn::Name.eq(&name))
                        .add(EventTypeColumn::Id.ne(id)),
                )
                .one(&ctx)
                .await?
            {
                return Err(EventTypeDaoError::AlreadyExists);
            }
            event_type_active.name = Set(name);
        }

        let updated_event_type = event_type_active.update(&ctx).await?;
        ctx.submit().await?;
        Ok(updated_event_type.into())
    }

    pub async fn delete(&self, id: i32) -> Result<(), EventTypeDaoError> {
        let ctx = self.db.get_transaction().await?;
        EventTypeEntity::delete_by_id(id).exec(&ctx).await?;
        ctx.submit().await?;
        Ok(())
    }
}

impl EventTypeDao {
    pub async fn find_by_name(
        &self, name: &str,
    ) -> Result<EventTypeResponse, EventTypeDaoError> {
        let ctx = self.db.get_transaction().await?;
        let condition = EventTypeColumn::Name.eq(name).into_condition();
        let result = self.query_one(condition, &ctx).await;
        ctx.submit().await?;
        result
    }
}
