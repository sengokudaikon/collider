#[macro_export]
macro_rules! redis_key {
    (hash $name:ident::<$t:ty> => $format_key:literal[$($arg:ident:$ty:ident),*])=>{
        #[doc=concat!(concat!("Redis Hash type binding \n ## Key \n", $format_key), concat!("\n ## Value Type \n ", stringify!($t)))]
        pub struct $name;

        impl $crate::infrastructure::cache::key::CacheKey for $name {
            type Args<'r> = ($(&'r $ty,)*);

            fn get_key_with_args(&self, args: Self::Args<'_>) -> std::borrow::Cow<'static, str> {
                let ($($arg,)*) = args;

                (format!($format_key, $($arg),*)).into()
            }
        }

        impl $crate::infrastructure::cache::type_bind::RedisTypeBind for $name {
            type RedisType<'redis, R> = $crate::cache::hash::Hash<'redis, R, $t>
                where
                    R: 'redis;
        }
    };
    (hash $name:ident::<$t:ty> => $key:literal) => {
        #[doc=concat!(concat!("Redis Hash type binding\n ## Key \n", $key), concat!("\n ## Value Type \n ", stringify!($t)))]
        pub struct $name;

        impl $crate::infrastructure::cache::key::CacheKey for $name {
            type Args<'r> = ();

            fn get_key_with_args(&self, _: Self::Args<'_>) -> std::borrow::Cow<'static, str> {
                ($key).into()
            }
        }

        impl $crate::infrastructure::cache::type_bind::RedisTypeBind for $name {
            type RedisType<'redis, R> = $crate::infrastructure::cache::hash::Hash<'redis, R, $t>
                where
                    R: 'redis;
        }
    };
    (tier $name:ident::<$t:ty> => $key:literal) => {
        #[doc=concat!(concat!("Redis + Memory type binding\n ## Key \n", $key), concat!("\n ## Value Type \n ", stringify!($t)))]
        pub struct $name;

        impl $crate::infrastructure::cache::key::CacheKey for $name {
            type Args<'r> = ()
            fn get_key_with_args(&self, _: Self::Args<'_>) -> std::borrow::Cow<'static, str> {
                ($key).into()
            }
        }

        impl $crate::infrastructure::cache::type_bind::RedisTypeBind for $name {
            type RedisType<'redis, R> = $crate::infrastructure::cache::tiered::Tiered<'redis, R, $t>
            where
                R: 'redis;
        }
    };
    (tier $name:ident::<$t:ty> => $format_key:literal[$($arg:ident:$ty:ident),*])=>{
        #[doc=concat!(concat!("Redis + Memory type binding\n ## Key \n", $format_key), concat!("\n ## Value Type \n ", stringify!($t)))]
        pub struct $name;

        impl $crate::infrastructure::cache::key::CacheKey for $name {
            type Args<'r> = ($(&'r $ty,)*);

            fn get_key_with_args(&self, args: Self::Args<'_>) -> std::borrow::Cow<'static, str> {
                let ($($arg,)*) = args;

                (format!($format_key, $($arg),*)).into()
            }
        }

        impl $crate::infrastructure::cache::key::CacheKey for $name {
            type RedisType<'redis, R> = $crate::infrastructure::cache::tiered::Tiered<'redis, R, $t>
            where
                R: 'redis
        }
    };
    (mem $name:ident::<$t:ty> => $key:literal) => {
        #[doc=concat!(concat!("Memory type binding\n ## Key \n", $key), concat!("\n ## Value Type \n ", stringify!($t)))]
        pub struct $name;

        impl $crate::infrastructure::cache::key::CacheKey for $name {
            type Args<'r> = ()
            fn get_key_with_args(&self, _: Self::Args<'_>) -> std::borrow::Cow<'static, str> {
                ($key).into()
            }
        }

        impl $crate::infrastructure::cache::type_bind::RedisTypeBind for $name {
            type RedisType<'redis, R> = $crate::infrastructure::cache::memory::Memory<>'redis, R, $t>
            where
                R: 'redis;
        }
    };
    (mem $name:ident::<$t:ty> => $format_key:literal[$($arg:ident:$ty:ident),*])=>{
        #[doc=concat!(concat!("Memory type binding\n ## Key \n", $format_key), concat!("\n ## Value Type \n ", stringify!($t)))]
        pub struct $name;

        impl $crate::infrastructure::cache::key::CacheKey for $name {
            type Args<'r> = ($(&'r $ty,)*);

            fn get_key_with_args(&self, args: Self::Args<'_>) -> std::borrow::Cow<'static, str> {
                let ($($arg,)*) = args;

                (format!($format_key, $($arg),*)).into()
            }
        }

        impl $crate::infrastructure::cache::key::CacheKey for $name {
            type RedisType<'redis, R> = $crate::infrastructure::cache::memory::Memory<'redis, R, $t>
            where
                R: 'redis
        }
    };
    ($name:ident::<$t:ty> => $format_key:literal[$($arg:ident:$ty:ident),*])=>{
        #[doc=concat!(concat!("Redis common type binding\n ## Key \n", $format_key), concat!("\n ## Value Type \n ", stringify!($t)))]
        pub struct $name;

        impl $crate::infrastructure::cache::key::CacheKey for $name {
            type Args<'r> = ($(&'r $ty,)*);

            fn get_key_with_args(&self, args: Self::Args<'_>) -> std::borrow::Cow<'static, str> {
                let ($($arg,)*) = args;

                (format!($format_key, $($arg),*)).into()
            }
        }

        impl $crate::infrastructure::cache::type_bind::RedisTypeBind for $name {
            type RedisType<'redis, R> = $crate::infrastructure::cache::normal::Normal<'redis, R, $t>
                where
                    R: 'redis;
        }
    };
    ($name:ident::<$t:ty> => $key:literal) => {
        #[doc=concat!(concat!("Redis common type binding\n ## Key \n", $key), concat!("\n ## Value Type \n ", stringify!($t)))]
        pub struct $name;

        impl $crate::infrastructure::cache::key::CacheKey for $name {
            type Args<'r> = ();

            fn get_key_with_args(&self, _: Self::Args<'_>) -> std::borrow::Cow<'static, str> {
                ($key).into()
            }
        }

        impl $crate::infrastructure::cache::type_bind::RedisTypeBind for $name {
            type RedisType<'redis, R> = $crate::infrastructure::cache::normal::Normal<'redis, R, $t>
                where
                    R: 'redis;
        }
    };
}
