#[macro_export]
macro_rules! redis_key {
    (hash $name:ident::<$t:ty> => $format_key:literal[$($arg:ident:$ty:ident),*])=>{
        #[doc=concat!(concat!("Redis Hash type binding \n ## Key \n", $format_key), concat!("\n ## Value Type \n ", stringify!($t)))]
        pub struct $name;

        impl $crate::core::key::CacheKey for $name {
            type Args<'r> = ($(&'r $ty,)*);

            fn get_key_with_args(&self, args: Self::Args<'_>) -> std::borrow::Cow<'static, str> {
                let ($($arg,)*) = args;

                (format!($format_key, $($arg),*)).into()
            }
        }

        impl $crate::core::type_bind::RedisTypeBind for $name {
            type RedisType<'redis, R> = $crate::types::hash::Hash<'redis, R, $t>
                where
                    R: 'redis;
        }
    };
    (hash $name:ident::<$t:ty> => $key:literal) => {
        #[doc=concat!(concat!("Redis Hash type binding\n ## Key \n", $key), concat!("\n ## Value Type \n ", stringify!($t)))]
        pub struct $name;

        impl $crate::core::key::CacheKey for $name {
            type Args<'r> = ();

            fn get_key_with_args(&self, _: Self::Args<'_>) -> std::borrow::Cow<'static, str> {
                ($key).into()
            }
        }

        impl $crate::core::type_bind::RedisTypeBind for $name {
            type RedisType<'redis, R> = $crate::types::hash::Hash<'redis, R, $t>
                where
                    R: 'redis;
        }
    };
    ($name:ident::<$t:ty> => $format_key:literal[$($arg:ident:$ty:ident),*])=>{
        #[doc=concat!(concat!("Redis common type binding\n ## Key \n", $format_key), concat!("\n ## Value Type \n ", stringify!($t)))]
        pub struct $name;

        impl $crate::core::key::CacheKey for $name {
            type Args<'r> = ($(&'r $ty,)*);

            fn get_key_with_args(&self, args: Self::Args<'_>) -> std::borrow::Cow<'static, str> {
                let ($($arg,)*) = args;

                (format!($format_key, $($arg),*)).into()
            }
        }

        impl $crate::core::type_bind::RedisTypeBind for $name {
            type RedisType<'redis, R> = $crate::types::normal::Normal<'redis, R, $t>
                where
                    R: 'redis;
        }
    };
    ($name:ident::<$t:ty> => $key:literal) => {
        #[doc=concat!(concat!("Redis common type binding\n ## Key \n", $key), concat!("\n ## Value Type \n ", stringify!($t)))]
        pub struct $name;

        impl $crate::core::key::CacheKey for $name {
            type Args<'r> = ();

            fn get_key_with_args(&self, _: Self::Args<'_>) -> std::borrow::Cow<'static, str> {
                ($key).into()
            }
        }

        impl $crate::core::type_bind::RedisTypeBind for $name {
            type RedisType<'redis, R> = $crate::types::normal::Normal<'redis, R, $t>
                where
                    R: 'redis;
        }
    };
    // Redis Set support
    (set $name:ident::<$t:ty> => $format_key:literal[$($arg:ident:$ty:ident),*])=>{
        #[doc=concat!(concat!("Redis Set type binding \n ## Key \n", $format_key), concat!("\n ## Value Type \n ", stringify!($t)))]
        pub struct $name;

        impl $crate::core::key::CacheKey for $name {
            type Args<'r> = ($(&'r $ty,)*);

            fn get_key_with_args(&self, args: Self::Args<'_>) -> std::borrow::Cow<'static, str> {
                let ($($arg,)*) = args;

                (format!($format_key, $($arg),*)).into()
            }
        }

        impl $crate::core::type_bind::RedisTypeBind for $name {
            type RedisType<'redis, R> = $crate::types::set::Set<'redis, R, $t>
                where
                    R: 'redis;
        }
    };
    (set $name:ident::<$t:ty> => $key:literal) => {
        #[doc=concat!(concat!("Redis Set type binding\n ## Key \n", $key), concat!("\n ## Value Type \n ", stringify!($t)))]
        pub struct $name;

        impl $crate::core::key::CacheKey for $name {
            type Args<'r> = ();

            fn get_key_with_args(&self, _: Self::Args<'_>) -> std::borrow::Cow<'static, str> {
                ($key).into()
            }
        }

        impl $crate::core::type_bind::RedisTypeBind for $name {
            type RedisType<'redis, R> = $crate::types::set::Set<'redis, R, $t>
                where
                    R: 'redis;
        }
    };
    // Redis Sorted Set (ZSet) support
    (zset $name:ident::<$t:ty> => $format_key:literal[$($arg:ident:$ty:ident),*])=>{
        #[doc=concat!(concat!("Redis Sorted Set type binding \n ## Key \n", $format_key), concat!("\n ## Value Type \n ", stringify!($t)))]
        pub struct $name;

        impl $crate::core::key::CacheKey for $name {
            type Args<'r> = ($(&'r $ty,)*);

            fn get_key_with_args(&self, args: Self::Args<'_>) -> std::borrow::Cow<'static, str> {
                let ($($arg,)*) = args;

                (format!($format_key, $($arg),*)).into()
            }
        }

        impl $crate::core::type_bind::RedisTypeBind for $name {
            type RedisType<'redis, R> = $crate::types::zset::SortedSet<'redis, R, $t>
                where
                    R: 'redis;
        }
    };
    (zset $name:ident::<$t:ty> => $key:literal) => {
        #[doc=concat!(concat!("Redis Sorted Set type binding\n ## Key \n", $key), concat!("\n ## Value Type \n ", stringify!($t)))]
        pub struct $name;

        impl $crate::core::key::CacheKey for $name {
            type Args<'r> = ();

            fn get_key_with_args(&self, _: Self::Args<'_>) -> std::borrow::Cow<'static, str> {
                ($key).into()
            }
        }

        impl $crate::core::type_bind::RedisTypeBind for $name {
            type RedisType<'redis, R> = $crate::types::zset::SortedSet<'redis, R, $t>
                where
                    R: 'redis;
        }
    };
    // Redis List support
    (list $name:ident::<$t:ty> => $format_key:literal[$($arg:ident:$ty:ident),*])=>{
        #[doc=concat!(concat!("Redis List type binding \n ## Key \n", $format_key), concat!("\n ## Value Type \n ", stringify!($t)))]
        pub struct $name;

        impl $crate::core::key::CacheKey for $name {
            type Args<'r> = ($(&'r $ty,)*);

            fn get_key_with_args(&self, args: Self::Args<'_>) -> std::borrow::Cow<'static, str> {
                let ($($arg,)*) = args;

                (format!($format_key, $($arg),*)).into()
            }
        }

        impl $crate::core::type_bind::RedisTypeBind for $name {
            type RedisType<'redis, R> = $crate::types::list::List<'redis, R, $t>
                where
                    R: 'redis;
        }
    };
    (list $name:ident::<$t:ty> => $key:literal) => {
        #[doc=concat!(concat!("Redis List type binding\n ## Key \n", $key), concat!("\n ## Value Type \n ", stringify!($t)))]
        pub struct $name;

        impl $crate::core::key::CacheKey for $name {
            type Args<'r> = ();

            fn get_key_with_args(&self, _: Self::Args<'_>) -> std::borrow::Cow<'static, str> {
                ($key).into()
            }
        }

        impl $crate::core::type_bind::RedisTypeBind for $name {
            type RedisType<'redis, R> = $crate::types::list::List<'redis, R, $t>
                where
                    R: 'redis;
        }
    };
    // Redis Stream support
    (stream $name:ident::<$t:ty> => $format_key:literal[$($arg:ident:$ty:ident),*])=>{
        #[doc=concat!(concat!("Redis Stream type binding \n ## Key \n", $format_key), concat!("\n ## Value Type \n ", stringify!($t)))]
        pub struct $name;

        impl $crate::core::key::CacheKey for $name {
            type Args<'r> = ($(&'r $ty,)*);

            fn get_key_with_args(&self, args: Self::Args<'_>) -> std::borrow::Cow<'static, str> {
                let ($($arg,)*) = args;

                (format!($format_key, $($arg),*)).into()
            }
        }

        impl $crate::core::type_bind::RedisTypeBind for $name {
            type RedisType<'redis, R> = $crate::types::stream::Stream<'redis, R, $t>
                where
                    R: 'redis;
        }
    };
    (stream $name:ident::<$t:ty> => $key:literal) => {
        #[doc=concat!(concat!("Redis Stream type binding\n ## Key \n", $key), concat!("\n ## Value Type \n ", stringify!($t)))]
        pub struct $name;

        impl $crate::core::key::CacheKey for $name {
            type Args<'r> = ();

            fn get_key_with_args(&self, _: Self::Args<'_>) -> std::borrow::Cow<'static, str> {
                ($key).into()
            }
        }

        impl $crate::core::type_bind::RedisTypeBind for $name {
            type RedisType<'redis, R> = $crate::types::stream::Stream<'redis, R, $t>
                where
                    R: 'redis;
        }
    };
}
