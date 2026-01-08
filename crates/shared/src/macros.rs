#[macro_export]
macro_rules! bfp {
    ($val:literal) => {
        ($val)
            .parse::<$crate::sources::balancer_v2::swap::fixed_point::Bfp>()
            .unwrap()
    };
}

#[macro_export]
macro_rules! json_map {
    ($($key:expr_2021 => $value:expr_2021),* $(,)?) => {{
        #[allow(unused_mut)]
        let mut map = ::serde_json::Map::<String, ::serde_json::Value>::new();
        $(
            map.insert(($key).into(), ($value).into());
        )*
        map
    }}
}

#[macro_export]
macro_rules! externalprices {
    (native_token: $nt:expr_2021 $(, $($t:tt)*)?) => {
        $crate::external_prices::ExternalPrices::try_new(
            $nt,
            ::maplit::hashmap!($($($t)*)*),
        )
        .unwrap()
    };
}
