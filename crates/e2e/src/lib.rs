// We export all the modules in this file so that we can use them in the tests
// and so that other crates can use them as well.

#[macro_use]
pub mod setup;
pub mod api;
pub mod nodes;

#[macro_export]
macro_rules! eth {
    ($amount:literal) => {
        ::alloy::primitives::U256::from($amount) * ::alloy::primitives::utils::Unit::ETHER.wei()
    };
}
