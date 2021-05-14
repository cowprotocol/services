#[macro_export]
macro_rules! addr {
    ($val:literal) => {
        ::ethcontract::H160(::hex_literal::hex!($val))
    };
}
