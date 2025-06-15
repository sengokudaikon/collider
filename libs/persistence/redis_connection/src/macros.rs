#[macro_export]
macro_rules! cache_key {
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

        impl $crate::core::type_bind::CacheTypeBind for $name {
            type CacheType<'cache> = $crate::types::hash::Hash<$t>;
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

        impl $crate::core::type_bind::CacheTypeBind for $name {
            type CacheType<'cache> = $crate::types::hash::Hash<$t>;
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

        impl $crate::core::type_bind::CacheTypeBind for $name {
            type CacheType<'cache> = $crate::types::normal::Normal<$t>;
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

        impl $crate::core::type_bind::CacheTypeBind for $name {
            type CacheType<'cache> = $crate::types::normal::Normal<$t>;
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

        impl $crate::core::type_bind::CacheTypeBind for $name {
            type CacheType<'cache> = $crate::types::set::Set<$t>;
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

        impl $crate::core::type_bind::CacheTypeBind for $name {
            type CacheType<'cache> = $crate::types::set::Set<$t>;
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

        impl $crate::core::type_bind::CacheTypeBind for $name {
            type CacheType<'cache> = $crate::types::zset::SortedSet<$t>;
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

        impl $crate::core::type_bind::CacheTypeBind for $name {
            type CacheType<'cache> = $crate::types::zset::SortedSet<$t>;
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

        impl $crate::core::type_bind::CacheTypeBind for $name {
            type CacheType<'cache> = $crate::types::list::List<$t>;
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

        impl $crate::core::type_bind::CacheTypeBind for $name {
            type CacheType<'cache> = $crate::types::list::List<$t>;
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

        impl $crate::core::type_bind::CacheTypeBind for $name {
            type CacheType<'cache> = $crate::types::stream::Stream<$t>;
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

        impl $crate::core::type_bind::CacheTypeBind for $name {
            type CacheType<'cache> = $crate::types::stream::Stream<$t>;
        }
    };
}
