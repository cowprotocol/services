#[macro_export]
macro_rules! addr {
    ($val:literal) => {
        ::ethcontract::H160(::hex_literal::hex!($val))
    };
}

#[macro_export]
macro_rules! bfp {
    ($val:literal) => {
        ($val)
            .parse::<$crate::sources::balancer_v2::swap::fixed_point::Bfp>()
            .unwrap()
    };
}

#[macro_export]
macro_rules! bytes {
    ($x:literal) => {
        ::ethcontract::web3::types::Bytes(::hex_literal::hex!($x).to_vec())
    };
}

#[macro_export]
macro_rules! json_map {
    ($($key:expr => $value:expr),* $(,)?) => {{
        #[allow(unused_mut)]
        let mut map = ::serde_json::Map::<String, ::serde_json::Value>::new();
        $(
            map.insert(($key).into(), ($value).into());
        )*
        map
    }}
}

#[macro_export]
macro_rules! dummy_contract {
    ($contract:ident, $addr:expr) => {
        $contract::at(&$crate::transport::dummy::web3(), $addr.into())
    };
}
