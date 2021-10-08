pub mod balancer;
pub mod offchain_orderbook;
pub mod slippage;
pub mod uniswap;

use crate::settlement::SettlementEncoder;
use anyhow::Result;
#[cfg(test)]
use derivative::Derivative;
#[cfg(test)]
use model::order::Order;
use model::{order::OrderKind, TokenPair};
use num::{rational::Ratio, BigRational};
use primitive_types::{H160, U256};
use shared::sources::balancer::pool_fetching::{
    AmplificationParameter, TokenState, WeightedTokenState,
};
#[cfg(test)]
use shared::sources::uniswap::pool_fetching::Pool;
use std::collections::HashMap;
use std::sync::Arc;
use strum_macros::{AsStaticStr, EnumVariantNames};

/// Defines the different types of liquidity our solvers support
#[derive(Clone, AsStaticStr, EnumVariantNames, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub enum Liquidity {
    ConstantProduct(ConstantProductOrder),
    BalancerWeighted(WeightedProductOrder),
    BalancerStable(StablePoolOrder),
}

impl Liquidity {
    /// Returns an iterator over all token pairs for the given liquidity.
    pub fn all_token_pairs(&self) -> Vec<TokenPair> {
        match self {
            Liquidity::ConstantProduct(amm) => vec![amm.tokens],
            Liquidity::BalancerWeighted(amm) => token_pairs(&amm.reserves),
            Liquidity::BalancerStable(amm) => token_pairs(&amm.reserves),
        }
    }
}

/// A trait associating some liquidity model to how it is executed and encoded
/// in a settlement (through a `SettlementHandling` reference). This allows
/// different liquidity types to be modeled the same way.
pub trait Settleable {
    type Execution;

    fn settlement_handling(&self) -> &dyn SettlementHandling<Self>;
}

/// Specifies how a liquidity execution gets encoded into a settlement.
pub trait SettlementHandling<L>: Send + Sync
where
    L: Settleable,
{
    fn encode(&self, execution: L::Execution, encoder: &mut SettlementEncoder) -> Result<()>;
}

/// Basic limit sell and buy orders
#[derive(Clone)]
pub struct LimitOrder {
    // Opaque Identifier for debugging purposes
    pub id: String,
    pub sell_token: H160,
    pub buy_token: H160,
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub kind: OrderKind,
    pub partially_fillable: bool,
    /// The scaled fee amount that the protocol pretends it is receiving.
    ///
    /// This is different than the actual order `fee_amount` value in that it
    /// does not have any subsidies applied and may be scaled by a constant
    /// factor to make matching orders more valuable from an objective value
    /// perspective.
    pub scaled_fee_amount: U256,
    pub is_liquidity_order: bool,
    pub settlement_handling: Arc<dyn SettlementHandling<Self>>,
}

impl std::fmt::Debug for LimitOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Limit Order {}", self.id)
    }
}

impl LimitOrder {
    /// Returns the full execution amount for the specified limit order.
    pub fn full_execution_amount(&self) -> U256 {
        match self.kind {
            OrderKind::Sell => self.sell_amount,
            OrderKind::Buy => self.buy_amount,
        }
    }
}

impl Settleable for LimitOrder {
    type Execution = U256;

    fn settlement_handling(&self) -> &dyn SettlementHandling<Self> {
        &*self.settlement_handling
    }
}

#[cfg(test)]
impl From<Order> for LimitOrder {
    fn from(order: Order) -> Self {
        offchain_orderbook::OrderConverter::test(H160([0x42; 20])).normalize_limit_order(order)
    }
}

#[cfg(test)]
impl Default for LimitOrder {
    fn default() -> Self {
        LimitOrder {
            sell_token: Default::default(),
            buy_token: Default::default(),
            sell_amount: Default::default(),
            buy_amount: Default::default(),
            kind: Default::default(),
            partially_fillable: Default::default(),
            scaled_fee_amount: Default::default(),
            settlement_handling: tests::CapturingSettlementHandler::arc(),
            is_liquidity_order: false,
            id: Default::default(),
        }
    }
}

/// 2 sided constant product automated market maker with equal reserve value and a trading fee (e.g. Uniswap, Sushiswap)
#[derive(Clone)]
#[cfg_attr(test, derive(Derivative))]
#[cfg_attr(test, derivative(PartialEq))]
pub struct ConstantProductOrder {
    pub tokens: TokenPair,
    pub reserves: (u128, u128),
    pub fee: Ratio<u32>,
    #[cfg_attr(test, derivative(PartialEq = "ignore"))]
    pub settlement_handling: Arc<dyn SettlementHandling<Self>>,
}

impl std::fmt::Debug for ConstantProductOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Constant Product AMM {:?}", self.tokens)
    }
}

#[cfg(test)]
impl From<Pool> for ConstantProductOrder {
    fn from(pool: Pool) -> Self {
        Self {
            tokens: pool.tokens,
            reserves: pool.reserves,
            fee: pool.fee,
            settlement_handling: tests::CapturingSettlementHandler::arc(),
        }
    }
}

/// 2 sided weighted product automated market maker with weighted reserves and a trading fee (e.g. BalancerV2)
#[derive(Clone)]
#[cfg_attr(test, derive(Derivative))]
#[cfg_attr(test, derivative(PartialEq))]
pub struct WeightedProductOrder {
    pub reserves: HashMap<H160, WeightedTokenState>,
    pub fee: BigRational,
    #[cfg_attr(test, derivative(PartialEq = "ignore"))]
    pub settlement_handling: Arc<dyn SettlementHandling<Self>>,
}

impl std::fmt::Debug for WeightedProductOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Weighted Product AMM {:?}", self.reserves.keys())
    }
}

#[derive(Clone)]
#[cfg_attr(test, derive(Derivative))]
#[cfg_attr(test, derivative(PartialEq))]
pub struct StablePoolOrder {
    pub reserves: HashMap<H160, TokenState>,
    pub fee: BigRational,
    pub amplification_parameter: AmplificationParameter,
    #[cfg_attr(test, derivative(PartialEq = "ignore"))]
    pub settlement_handling: Arc<dyn SettlementHandling<Self>>,
}

impl std::fmt::Debug for StablePoolOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Stable Pool AMM {:?}", self.reserves.keys())
    }
}

pub fn token_pairs<T>(reserves: &HashMap<H160, T>) -> Vec<TokenPair> {
    // The `HashMap` docs specifically say that we can't rely on ordering
    // of keys (even across multiple calls). So, first collect all tokens
    // into a collection and then use it to make the final enumeration with
    // all token pair permutations.
    let tokens = reserves.keys().collect::<Vec<_>>();
    tokens
        .iter()
        .enumerate()
        .flat_map(|(i, &token_a)| {
            tokens[i + 1..].iter().map(move |&token_b| {
                TokenPair::new(*token_a, *token_b).expect("unexpected duplicate key in hash map")
            })
        })
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AmmOrderExecution {
    pub input: (H160, U256),
    pub output: (H160, U256),
}

impl ConstantProductOrder {
    pub fn constant_product(&self) -> U256 {
        U256::from(self.reserves.0) * U256::from(self.reserves.1)
    }
}

impl Settleable for ConstantProductOrder {
    type Execution = AmmOrderExecution;

    fn settlement_handling(&self) -> &dyn SettlementHandling<Self> {
        &*self.settlement_handling
    }
}

impl Settleable for WeightedProductOrder {
    type Execution = AmmOrderExecution;

    fn settlement_handling(&self) -> &dyn SettlementHandling<Self> {
        &*self.settlement_handling
    }
}

impl Settleable for StablePoolOrder {
    type Execution = AmmOrderExecution;

    fn settlement_handling(&self) -> &dyn SettlementHandling<Self> {
        &*self.settlement_handling
    }
}

#[cfg(test)]
impl Default for ConstantProductOrder {
    fn default() -> Self {
        ConstantProductOrder {
            tokens: Default::default(),
            reserves: Default::default(),
            fee: num::Zero::zero(),
            settlement_handling: tests::CapturingSettlementHandler::arc(),
        }
    }
}

#[cfg(test)]
impl Default for WeightedProductOrder {
    fn default() -> Self {
        WeightedProductOrder {
            reserves: Default::default(),
            fee: num::Zero::zero(),
            settlement_handling: tests::CapturingSettlementHandler::arc(),
        }
    }
}

#[cfg(test)]
impl Default for StablePoolOrder {
    fn default() -> Self {
        StablePoolOrder {
            reserves: Default::default(),
            fee: num::Zero::zero(),
            amplification_parameter: AmplificationParameter::new(1.into(), 1.into()).unwrap(),
            settlement_handling: tests::CapturingSettlementHandler::arc(),
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use maplit::hashmap;
    use std::sync::Mutex;

    pub struct CapturingSettlementHandler<L>
    where
        L: Settleable,
    {
        pub calls: Mutex<Vec<L::Execution>>,
    }

    // Manual implementation seems to be needed as `derive(Default)` adds an
    // unneeded `L::Execution: Default` type bound.
    impl<L> Default for CapturingSettlementHandler<L>
    where
        L: Settleable,
    {
        fn default() -> Self {
            Self {
                calls: Default::default(),
            }
        }
    }

    impl<L> CapturingSettlementHandler<L>
    where
        L: Settleable,
        L::Execution: Clone,
    {
        pub fn arc() -> Arc<Self> {
            Arc::new(Default::default())
        }

        pub fn calls(&self) -> Vec<L::Execution> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl<L> SettlementHandling<L> for CapturingSettlementHandler<L>
    where
        L: Settleable,
        L::Execution: Send + Sync,
    {
        fn encode(&self, execution: L::Execution, _: &mut SettlementEncoder) -> Result<()> {
            self.calls.lock().unwrap().push(execution);
            Ok(())
        }
    }

    #[test]
    fn limit_order_full_execution_amounts() {
        fn simple_limit_order(
            kind: OrderKind,
            sell_amount: impl Into<U256>,
            buy_amount: impl Into<U256>,
        ) -> LimitOrder {
            LimitOrder {
                id: Default::default(),
                sell_token: Default::default(),
                buy_token: Default::default(),
                sell_amount: sell_amount.into(),
                buy_amount: buy_amount.into(),
                kind,
                partially_fillable: Default::default(),
                scaled_fee_amount: Default::default(),
                settlement_handling: CapturingSettlementHandler::arc(),
                is_liquidity_order: false,
            }
        }

        assert_eq!(
            simple_limit_order(OrderKind::Sell, 1, 2).full_execution_amount(),
            1.into(),
        );
        assert_eq!(
            simple_limit_order(OrderKind::Buy, 1, 2).full_execution_amount(),
            2.into(),
        );
    }

    #[test]
    fn enumerate_token_pairs() {
        let token_map: HashMap<_, Option<u32>> = hashmap! {
            H160([0x11; 20]) => None,
            H160([0x22; 20]) => None,
            H160([0x33; 20]) => None,
            H160([0x44; 20]) => None,
        };
        let mut pairs = token_pairs(&token_map);
        pairs.sort();

        assert_eq!(
            pairs,
            vec![
                TokenPair::new(H160([0x11; 20]), H160([0x22; 20])).unwrap(),
                TokenPair::new(H160([0x11; 20]), H160([0x33; 20])).unwrap(),
                TokenPair::new(H160([0x11; 20]), H160([0x44; 20])).unwrap(),
                TokenPair::new(H160([0x22; 20]), H160([0x33; 20])).unwrap(),
                TokenPair::new(H160([0x22; 20]), H160([0x44; 20])).unwrap(),
                TokenPair::new(H160([0x33; 20]), H160([0x44; 20])).unwrap(),
            ]
        );
    }
}
