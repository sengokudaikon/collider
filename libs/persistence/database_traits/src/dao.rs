use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};

#[async_trait]
pub trait GenericDao {
    type Model: Send + Sync + 'static;
    type Response: From<Self::Model> + Send + Sync + 'static;
    type CreateRequest: Send + Sync + 'static;
    type UpdateRequest: Send + Sync + 'static;
    type Error: Send + 'static;
    type ID: Serialize + DeserializeOwned + Send + Sync + 'static;

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