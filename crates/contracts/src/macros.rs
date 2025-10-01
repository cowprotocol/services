#[macro_export]
macro_rules! dummy_contract {
    ($contract:ty, $addr:expr_2021) => {
        <$contract>::at(&$crate::web3::dummy(), $addr.into())
    };
}
