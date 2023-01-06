use crate::domain::eth;
use ethereum_types::U256;

/// The state of a Curve-like stable pool.
#[derive(Clone, Debug)]
pub struct Pool {
    pub reserves: Vec<Reserve>,
    pub amplification_parameter: eth::Rational,
    pub fee: eth::Rational,
}

/// A stable pool token reserve.
#[derive(Clone, Debug)]
pub struct Reserve {
    pub asset: eth::Asset,
    pub scaling_factor: U256,
}
