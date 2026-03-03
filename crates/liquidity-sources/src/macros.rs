#[macro_export]
macro_rules! bfp {
    ($val:literal) => {
        ($val)
            .parse::<$crate::balancer_v2::swap::fixed_point::Bfp>()
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
