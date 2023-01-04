//! Modelling on-chain liquidity.

pub mod concentrated;
pub mod constantproduct;
pub mod limitorder;
pub mod stable;
pub mod weighted;

use crate::domain::eth;

/// A source of liquidity which can be used by the solver.
#[derive(Debug, Clone, Copy)]
pub struct Liquidity {
    pub id: Id,
    /// Estimation of gas needed to use this liquidity on-chain.
    pub gas: eth::Gas,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Id(pub usize);

/// The liquidity state, specific to the type of liquidity.
pub enum State {
    ConstantProduct(constantproduct::Pool),
    WeightedProduct(weighted::Pool),
    Stable(stable::Pool),
    Concentrated(concentrated::Pool),
    LimitOrder(limitorder::LimitOrder),
}
