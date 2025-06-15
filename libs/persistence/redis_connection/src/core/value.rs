use std::ops::{Deref, DerefMut};

use deadpool_redis::redis::{
    ErrorKind, FromRedisValue, RedisError, RedisResult, RedisWrite,
    ToRedisArgs, Value,
};
use serde::{Deserialize, Serialize};

/// The unified trait for all cacheable values
pub trait CacheValue: Sized + Send + Sync {
    /// Serialize to bytes for any cache backend
    fn to_bytes(&self) -> Result<Vec<u8>, CacheError>;

    /// Deserialize from bytes
    fn from_bytes(bytes: &[u8]) -> Result<Self, CacheError>;
}

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("Serialization failed: {0}")]
    Serialization(String),
    #[error("Deserialization failed: {0}")]
    Deserialization(String),
    #[error("Invalid format")]
    InvalidFormat,
}

/// Json<T> is now our primary wrapper for complex types
/// It implements both CacheValue and Redis traits directly
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Json<T>(pub T);

impl<T> Json<T> {
    pub fn new(value: T) -> Self { Self(value) }

    pub fn inner(self) -> T { self.0 }

    pub fn as_inner(&self) -> &T { &self.0 }

    pub fn as_inner_mut(&mut self) -> &mut T { &mut self.0 }
}

// Implement Deref for ergonomic access
impl<T> Deref for Json<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target { &self.0 }
}

impl<T> DerefMut for Json<T> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}

// Implement From for easy conversion
impl<T> From<T> for Json<T> {
    fn from(value: T) -> Self { Json(value) }
}

impl<T> CacheValue for Json<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Send + Sync,
{
    fn to_bytes(&self) -> Result<Vec<u8>, CacheError> {
        serde_json::to_vec(&self.0)
            .map_err(|e| CacheError::Serialization(e.to_string()))
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, CacheError> {
        serde_json::from_slice(bytes)
            .map(Json)
            .map_err(|e| CacheError::Deserialization(e.to_string()))
    }
}

impl<T> ToRedisArgs for Json<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Send + Sync,
{
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + RedisWrite,
    {
        match self.to_bytes() {
            Ok(bytes) => out.write_arg(&bytes),
            Err(_) => out.write_arg(b""),
        }
    }
}

impl<T> FromRedisValue for Json<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Send + Sync,
{
    fn from_redis_value(v: &Value) -> RedisResult<Self> {
        match v {
            Value::BulkString(data) => {
                Self::from_bytes(data).map_err(|e| {
                    RedisError::from((
                        ErrorKind::TypeError,
                        "JSON deserialization failed",
                        e.to_string(),
                    ))
                })
            }
            Value::Nil => {
                Err(RedisError::from((
                    ErrorKind::TypeError,
                    "Cannot convert nil to JSON value",
                )))
            }
            _ => {
                Err(RedisError::from((
                    ErrorKind::TypeError,
                    "Expected bulk string for JSON",
                )))
            }
        }
    }
}

// For primitive types, we can use a different wrapper that uses efficient
// encoding
#[derive(Clone, Debug)]
pub struct Primitive<T>(pub T);

impl<T> Primitive<T> {
    pub fn new(value: T) -> Self { Self(value) }

    pub fn inner(self) -> T { self.0 }
}

// Efficient implementations for primitives
impl CacheValue for Primitive<String> {
    fn to_bytes(&self) -> Result<Vec<u8>, CacheError> {
        Ok(self.0.as_bytes().to_vec())
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, CacheError> {
        String::from_utf8(bytes.to_vec())
            .map(Primitive)
            .map_err(|_| CacheError::InvalidFormat)
    }
}

impl CacheValue for Primitive<i64> {
    fn to_bytes(&self) -> Result<Vec<u8>, CacheError> {
        Ok(self.0.to_le_bytes().to_vec())
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, CacheError> {
        if bytes.len() != 8 {
            return Err(CacheError::InvalidFormat);
        }
        let mut arr = [0u8; 8];
        arr.copy_from_slice(bytes);
        Ok(Primitive(i64::from_le_bytes(arr)))
    }
}

// Direct Redis support for primitives
impl ToRedisArgs for Primitive<String> {
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + RedisWrite,
    {
        self.0.write_redis_args(out)
    }
}

impl FromRedisValue for Primitive<String> {
    fn from_redis_value(v: &Value) -> RedisResult<Self> {
        String::from_redis_value(v).map(Primitive)
    }
}

impl ToRedisArgs for Primitive<i64> {
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + RedisWrite,
    {
        self.0.write_redis_args(out)
    }
}

impl FromRedisValue for Primitive<i64> {
    fn from_redis_value(v: &Value) -> RedisResult<Self> {
        i64::from_redis_value(v).map(Primitive)
    }
}

// No more SerdeJson - Json<T> handles everything directly!

// Helper trait for easy conversion
pub trait IntoCacheValue: Sized {
    type Wrapped: CacheValue;

    fn into_cache_value(self) -> Self::Wrapped;
}

impl<T> IntoCacheValue for T
where
    T: Serialize + for<'de> Deserialize<'de> + Send + Sync,
{
    type Wrapped = Json<T>;

    fn into_cache_value(self) -> Self::Wrapped { Json(self) }
}

// Convenience for the RedisValue trait
pub trait RedisValue<'redis>:
    CacheValue + ToRedisArgs + FromRedisValue + 'redis
{
}

impl<'redis, T> RedisValue<'redis> for T where
    T: CacheValue + ToRedisArgs + FromRedisValue + 'redis
{
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
    struct User {
        id: u64,
        name: String,
    }

    #[test]
    fn test_json_roundtrip() {
        let user = User {
            id: 1,
            name: "Alice".into(),
        };
        let json = Json(user.clone());

        let bytes = json.to_bytes().unwrap();
        let recovered = Json::<User>::from_bytes(&bytes).unwrap();

        assert_eq!(recovered.0, user);
    }

    #[test]
    fn test_primitive_string() {
        let value = Primitive("Hello".to_string());
        let bytes = value.to_bytes().unwrap();
        let recovered = Primitive::<String>::from_bytes(&bytes).unwrap();

        assert_eq!(recovered.0, "Hello");
    }
}
