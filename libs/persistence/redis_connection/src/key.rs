use std::borrow::Cow;

pub trait CacheKey {
    type Args<'r>;

    #[allow(unused_variables)]
    fn get_key_with_args(&self, arg: Self::Args<'_>) -> Cow<'static, str>;

    #[allow(unused)]
    fn get_key(&self) -> Cow<'static, str>
    where
        for<'r> Self::Args<'r>: CacheKeyAutoConstruct,
    {
        CacheKey::get_key_with_args(self, CacheKeyAutoConstruct::construct())
    }
}

pub trait CacheKeyArg1 {
    type Arg0;

    fn construct(arg0: Self::Arg0) -> Self;
}

impl<T> CacheKeyArg1 for (T,) {
    type Arg0 = T;

    fn construct(arg0: Self::Arg0) -> Self { (arg0,) }
}

pub trait CacheKeyAutoConstruct {
    fn construct() -> Self;
}

impl CacheKeyAutoConstruct for () {
    fn construct() -> Self {}
}
