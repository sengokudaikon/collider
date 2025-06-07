use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, Condition, ConnectionTrait, DbErr, EntityTrait,
    ModelTrait,
};
use serde::{Serialize, de::DeserializeOwned};

#[async_trait]
pub trait GenericDao {
    type Entity: EntityTrait + Send + Sync + 'static;
    type Model: ModelTrait + Send + Sync + 'static;
    type ActiveModel: ActiveModelTrait<Entity = Self::Entity>
        + Send
        + Sync
        + 'static;
    type Response: From<Self::Model> + Send + Sync + 'static;
    type CreateRequest: Send + Sync + 'static;
    type UpdateRequest: Send + Sync + 'static;
    type Error: From<DbErr> + Send + 'static;
    type ID: Serialize + DeserializeOwned + Send + Sync + 'static;
    async fn query_one(
        &self,
        condition: impl Into<Option<Condition>> + Send + Sync + 'static,
        db: &impl ConnectionTrait,
    ) -> Result<Self::Response, Self::Error>;

    async fn query_all(
        &self,
        condition: impl Into<Option<Condition>> + Send + Sync + 'static,
        db: &impl ConnectionTrait,
    ) -> Result<Vec<Self::Response>, Self::Error>;

    async fn find_by_id(
        &self, id: Self::ID,
    ) -> Result<Self::Response, Self::Error>;

    async fn all(&self) -> Result<Vec<Self::Response>, Self::Error>;

    async fn create(
        &self, req: Self::CreateRequest,
    ) -> Result<Self::Response, Self::Error>;

    async fn update(
        &self, id: Self::ID, req: Self::UpdateRequest,
    ) -> Result<Self::Response, Self::Error>;

    async fn delete(&self, id: Self::ID) -> Result<(), Self::Error>;
}
