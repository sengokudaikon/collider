use std::marker::PhantomData;

use serde::Serialize;

#[derive(Clone)]
pub struct Json<T>(pub T);
#[derive(Clone)]
pub struct SerdeJson<T>(pub Vec<u8>, PhantomData<T>);
impl<T> Json<T> {
    pub fn serde(self) -> serde_json::Result<SerdeJson<T>>
    where
        T: Serialize,
    {
        Ok(SerdeJson(serde_json::to_vec(&self.0)?, PhantomData))
    }

    pub fn inner(self) -> T { self.0 }
}
