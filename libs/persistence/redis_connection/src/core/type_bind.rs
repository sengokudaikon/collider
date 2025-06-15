use std::borrow::Cow;

use super::{
    backend::CacheBackend,
    key::{CacheKey, CacheKeyArg1, CacheKeyAutoConstruct},
};

pub trait CacheTypeTrait<'cache>: Sized {
    fn from_cache_and_key(
        backend: CacheBackend<'cache>, key: Cow<'static, str>,
    ) -> Self;

    #[allow(unused)]
    fn clear(self) { drop(self) }
}

pub trait CacheTypeBind: CacheKey {
    type CacheType<'cache>: CacheTypeTrait<'cache>;

    fn bind_with_args<'cache>(
        &self, backend: impl Into<CacheBackend<'cache>>,
        args: <Self as CacheKey>::Args<'_>,
    ) -> Self::CacheType<'cache> {
        let key = CacheKey::get_key_with_args(self, args);
        CacheTypeTrait::from_cache_and_key(backend.into(), key)
    }

    fn bind_with<'cache>(
        &self, backend: impl Into<CacheBackend<'cache>>,
        arg: <<Self as CacheKey>::Args<'_> as CacheKeyArg1>::Arg0,
    ) -> Self::CacheType<'cache>
    where
        for<'r> <Self as CacheKey>::Args<'r>: CacheKeyArg1,
    {
        CacheTypeBind::bind_with_args(
            self,
            backend,
            <<Self as CacheKey>::Args<'_> as CacheKeyArg1>::construct(arg),
        )
    }

    fn bind<'cache>(
        &self, backend: impl Into<CacheBackend<'cache>>,
    ) -> Self::CacheType<'cache>
    where
        for<'r> <Self as CacheKey>::Args<'r>: CacheKeyAutoConstruct,
    {
        CacheTypeBind::bind_with_args(
            self,
            backend,
            CacheKeyAutoConstruct::construct(),
        )
    }
}
