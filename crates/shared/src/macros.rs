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
        $contract::at(&$crate::ethrpc::dummy::web3(), $addr.into())
    };
}

#[macro_export]
macro_rules! bytecode {
    ($contract:ty) => {
        <$contract>::raw_contract().bytecode.to_bytes().unwrap()
    };
}

#[macro_export]
macro_rules! deployed_bytecode {
    ($contract:ty) => {
        <$contract>::raw_contract()
            .deployed_bytecode
            .to_bytes()
            .unwrap()
    };
}
