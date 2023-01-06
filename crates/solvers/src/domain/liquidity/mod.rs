//! Modelling on-chain liquidity.

pub mod balancer;
pub mod concentrated;
pub mod constantproduct;
pub mod limitorder;
pub mod stable;
pub mod weighted;

use crate::domain::eth;
use ethereum_types::H160;

/// A source of liquidity which can be used by the solver.
#[derive(Clone, Debug)]
pub struct Liquidity {
    pub id: Id,
    pub address: H160,
    /// Estimation of gas needed to use this liquidity on-chain.
    pub gas: eth::Gas,
    pub state: State,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Id(pub usize);

/// The liquidity state, specific to the type of liquidity.
#[derive(Clone, Debug)]
pub enum State {
    ConstantProduct(constantproduct::Pool),
    WeightedProduct(weighted::Pool),
    Stable(stable::Pool),
    Concentrated(concentrated::Pool),
    LimitOrder(limitorder::LimitOrder),
}
