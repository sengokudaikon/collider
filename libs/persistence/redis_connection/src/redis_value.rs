use deadpool_redis::redis::{
    ErrorKind, FromRedisValue, RedisError, RedisResult, RedisWrite,
    ToRedisArgs, Value,
};
use serde::{Deserialize, Serialize};

use super::json::{Json, SerdeJson};

pub trait RedisValue<'redis> {
    type Input: ToRedisArgs + Send + Sync + 'redis;

    type Output: FromRedisValue + Send + Sync + 'redis;
}

impl<'redis, T> RedisValue<'redis> for Json<T>
where
    T: for<'de> Deserialize<'de> + Serialize + 'static + Send + Sync,
    Self: Send + Sync + 'redis,
{
    type Input = SerdeJson<T>;
    type Output = Self;
}

impl<T: Serialize> ToRedisArgs for SerdeJson<T> {
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + RedisWrite,
    {
        self.0.write_redis_args(out)
    }
}

impl<T> FromRedisValue for Json<T>
where
    for<'de> T: Deserialize<'de> + 'static,
{
    fn from_redis_value(v: &Value) -> RedisResult<Self> {
        let Value::BulkString(data) = v
        else {
            return Err(deadpool_redis::redis::RedisError::from((
                ErrorKind::TypeError,
                "Expect Json String",
            )));
        };
        let payload = serde_json::from_slice(data).map_err(|err| {
            RedisError::from((
                ErrorKind::TypeError,
                "Cannot Deserialize Json String",
                err.to_string(),
            ))
        })?;

        Ok(Self(payload))
    }
}
