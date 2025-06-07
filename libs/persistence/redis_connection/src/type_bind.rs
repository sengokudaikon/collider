use std::borrow::Cow;

use bytes::Bytes;
use moka::future::Cache;

use crate::key::{CacheKey, CacheKeyArg1, CacheKeyAutoConstruct};

pub trait RedisTypeTrait<'redis, R>: Sized {
    fn from_redis_and_key(
        redis: &'redis mut R, key: Cow<'static, str>,
        memory: Option<Cache<String, Bytes>>,
    ) -> Self;
    #[allow(unused)]
    fn clear(self) { drop(self) }
}

pub trait RedisTypeBind: CacheKey {
    type RedisType<'redis, R>: RedisTypeTrait<'redis, R>
    where
        R: 'redis;
    /// Construct a Redis type binding, when key construction requires 2 or
    /// more parameters
    fn bind_with_args<'redis, R>(
        &self, redis: &'redis mut R, args: <Self as CacheKey>::Args<'_>,
    ) -> Self::RedisType<'redis, R>
    where
        R: 'redis,
    {
        let key = CacheKey::get_key_with_args(self, args);
        RedisTypeTrait::from_redis_and_key(redis, key, None)
    }
    /// Construct a Redis type binding, when key construction requires a
    /// parameter
    fn bind_with<'redis, R>(
        &self, redis: &'redis mut R,
        arg: <<Self as CacheKey>::Args<'_> as CacheKeyArg1>::Arg0,
    ) -> Self::RedisType<'redis, R>
    where
        for<'r> <Self as CacheKey>::Args<'r>: CacheKeyArg1,
    {
        RedisTypeBind::bind_with_args(
            self,
            redis,
            <<Self as CacheKey>::Args<'_> as CacheKeyArg1>::construct(arg),
        )
    }

    /// Construct a Redis type binding, when key construction does not require
    /// any parameters
    fn bind<'redis, R>(
        &self, redis: &'redis mut R,
    ) -> Self::RedisType<'redis, R>
    where
        R: 'redis,
        for<'r> <Self as CacheKey>::Args<'r>: CacheKeyAutoConstruct,
    {
        RedisTypeBind::bind_with_args(
            self,
            redis,
            CacheKeyAutoConstruct::construct(),
        )
    }

    fn bind_mem<'redis, R>(
        &self, redis: &'redis mut R, memory: Cache<String, Bytes>,
    ) -> Self::RedisType<'redis, R>
    where
        R: 'redis,
        for<'r> <Self as CacheKey>::Args<'r>: CacheKeyAutoConstruct,
    {
        let key = CacheKey::get_key_with_args(
            self,
            CacheKeyAutoConstruct::construct(),
        );
        RedisTypeTrait::from_redis_and_key(redis, key, Some(memory))
    }
    fn bind_mem_with<'redis, R>(
        &self, redis: &'redis mut R,
        arg: <<Self as CacheKey>::Args<'_> as CacheKeyArg1>::Arg0,
        memory: Cache<String, Bytes>,
    ) -> Self::RedisType<'redis, R>
    where
        for<'r> <Self as CacheKey>::Args<'r>: CacheKeyArg1,
    {
        let key = CacheKey::get_key_with_args(
            self,
            <<Self as CacheKey>::Args<'_> as CacheKeyArg1>::construct(arg),
        );
        RedisTypeTrait::from_redis_and_key(redis, key, Some(memory))
    }

    /// Construct a Redis type binding with memory cache
    fn bind_tiered<'redis, R>(
        &self, redis: &'redis mut R, memory: Cache<String, Bytes>,
    ) -> Self::RedisType<'redis, R>
    where
        R: 'redis,
        for<'r> <Self as CacheKey>::Args<'r>: CacheKeyAutoConstruct,
    {
        let key = CacheKey::get_key_with_args(
            self,
            CacheKeyAutoConstruct::construct(),
        );
        RedisTypeTrait::from_redis_and_key(redis, key, Some(memory))
    }

    fn bind_tiered_with<'redis, R>(
        &self, redis: &'redis mut R,
        arg: <<Self as CacheKey>::Args<'_> as CacheKeyArg1>::Arg0,
        memory: Cache<String, Bytes>,
    ) -> Self::RedisType<'redis, R>
    where
        for<'r> <Self as CacheKey>::Args<'r>: CacheKeyArg1,
    {
        let key = CacheKey::get_key_with_args(
            self,
            <<Self as CacheKey>::Args<'_> as CacheKeyArg1>::construct(arg),
        );
        RedisTypeTrait::from_redis_and_key(redis, key, Some(memory))
    }
}
