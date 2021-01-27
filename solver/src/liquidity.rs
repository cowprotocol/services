use model::{order::OrderKind, TokenPair};
use num_rational::Rational;
use primitive_types::{H160, U256};
use settlement::{Interaction, Trade};
use std::sync::Arc;

#[cfg(test)]
use mockall::automock;

use crate::settlement;

/// Defines the different types of liquidity our solvers support
pub enum Liquidity {
    Limit(LimitOrder),
    Amm(AmmOrder),
}
/// Basic limit sell and buy orders
#[derive(Clone)]
pub struct LimitOrder {
    pub sell_token: H160,
    pub buy_token: H160,
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub kind: OrderKind,
    pub partially_fillable: bool,
    pub settlement_handling: Arc<dyn LimitOrderSettlementHandling>,
}

/// Specifies how a limit order fulfillment translates into Trade and Interactions for the settlement
#[cfg_attr(test, automock)]
pub trait LimitOrderSettlementHandling: Send + Sync {
    fn settle(&self, executed_amount: U256) -> (Option<Trade>, Vec<Box<dyn Interaction>>);
}

/// 2 sided constant product automated market maker with equal reserve value and a trading fee (e.g. Uniswap, Sushiswap)
#[derive(Clone)]
pub struct AmmOrder {
    pub tokens: TokenPair,
    pub reserves: (u128, u128),
    pub fee: Rational,
    pub settlement_handling: Arc<dyn AmmSettlementHandling>,
}

/// Specifies how a AMM order fulfillment translates into Interactions for the settlement
#[cfg_attr(test, automock)]
pub trait AmmSettlementHandling: Send + Sync {
    fn settle(&self, input: (H160, U256), output: (H160, U256)) -> Vec<Box<dyn Interaction>>;
}
