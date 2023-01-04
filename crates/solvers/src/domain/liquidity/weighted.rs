use crate::domain::eth;

/// The state of a Balancer-like weighted product pool.
pub struct Pool {
    pub reserves: Vec<Reserve>,
    pub fee: eth::Rational,
}

/// A weighted pool token reserve.
pub struct Reserve {
    pub asset: eth::Asset,
    pub weight: eth::Rational,
}
