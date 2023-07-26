pub mod balancer_v2;
pub mod order_converter;
pub mod slippage;
pub mod uniswap_v2;
pub mod uniswap_v3;
pub mod zeroex;

#[cfg(test)]
use derivative::Derivative;
#[cfg(test)]
use model::order::Order;
use {
    crate::settlement::SettlementEncoder,
    anyhow::Result,
    model::{
        order::{OrderKind, OrderUid},
        TokenPair,
    },
    num::rational::Ratio,
    primitive_types::{H160, U256},
    shared::{
        http_solver::model::TokenAmount,
        sources::{
            balancer_v2::{
                pool_fetching::{AmplificationParameter, TokenState, WeightedTokenState},
                swap::fixed_point::Bfp,
            },
            uniswap_v2::pool_fetching::Pool,
            uniswap_v3::pool_fetching::PoolInfo,
        },
    },
    std::{collections::HashMap, sync::Arc},
    strum::{EnumVariantNames, IntoStaticStr},
};

/// Defines the different types of liquidity our solvers support
#[derive(Clone, IntoStaticStr, EnumVariantNames, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub enum Liquidity {
    ConstantProduct(ConstantProductOrder),
    BalancerWeighted(WeightedProductOrder),
    BalancerStable(StablePoolOrder),
    LimitOrder(LimitOrder),
    Concentrated(ConcentratedLiquidity),
}

impl Liquidity {
    /// Returns an iterator over all token pairs for the given liquidity.
    pub fn all_token_pairs(&self) -> Vec<TokenPair> {
        match self {
            Liquidity::ConstantProduct(amm) => vec![amm.tokens],
            Liquidity::BalancerWeighted(amm) => token_pairs(&amm.reserves),
            Liquidity::BalancerStable(amm) => token_pairs(&amm.reserves),
            Liquidity::LimitOrder(order) => TokenPair::new(order.sell_token, order.buy_token)
                .map(|pair| vec![pair])
                .unwrap_or_default(),
            Liquidity::Concentrated(amm) => vec![amm.tokens],
        }
    }

    /// Returns the pool address on the blockchain containing this liquidity
    pub fn address(&self) -> Option<H160> {
        match self {
            Liquidity::ConstantProduct(amm) => Some(amm.address),
            Liquidity::BalancerWeighted(amm) => Some(amm.address),
            Liquidity::BalancerStable(amm) => Some(amm.address),
            Liquidity::LimitOrder(_) => None,
            Liquidity::Concentrated(amm) => Some(amm.pool.address),
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
    /// What is this craziness?!
    ///
    /// While developing the `driver`, we want to access information that is
    /// part of a liquidity's settlement handler. Unfortunately, with how the
    /// `Liquidity` abstraction is currently setup, this is not really possible.
    /// This method allows us to downcast `SettlementHandling` trait objects
    /// into concrete types in order to make the `driver` boundary integration
    /// work.
    ///
    /// This should eventually be purged with fire.
    fn as_any(&self) -> &dyn std::any::Any;

    fn encode(&self, execution: L::Execution, encoder: &mut SettlementEncoder) -> Result<()>;
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Exchange {
    GnosisProtocol,
    ZeroEx,
}

/// Used to differentiate between different types of orders that can be sent to
/// solvers. User orders (market + limit) containing OrderUid are the orders
/// from the orderbook.
#[derive(Debug, Clone)]
#[cfg_attr(test, derive(Derivative))]
#[cfg_attr(test, derivative(PartialEq))]
pub enum LimitOrderId {
    Market(OrderUid),
    Limit(OrderUid),
    Liquidity(LiquidityOrderId),
}

/// Three different types of liquidity orders exist:
/// 1. Protocol - liquidity orders from the auction model of solvable orders
/// 2. ZeroEx  - liquidity orders from the zeroex api liquidity collector
/// 3. Foreign - liquidity orders received as part of the solution from
/// searchers
///
/// (1) and (2) are gathered when the auction is cut and they are sent to
/// searchers (3) are received from searchers as part of the solution.
#[derive(Debug, Clone)]
#[cfg_attr(test, derive(Derivative))]
#[cfg_attr(test, derivative(PartialEq))]
pub enum LiquidityOrderId {
    /// TODO: Split into different variants once we have a DTO of order model
    /// for `driver` in driver solver colocation TODO: The only reason why
    /// is together now is because function `normalize_limit_order` can't
    /// diferentiate between these two Contains protocol and foreign
    /// liquidity orders
    Protocol(OrderUid),
    ZeroEx(String),
}

#[cfg(test)]
impl Default for LimitOrderId {
    fn default() -> Self {
        Self::Market(Default::default())
    }
}

impl LimitOrderId {
    pub fn order_uid(&self) -> Option<OrderUid> {
        match self {
            LimitOrderId::Market(uid) => Some(*uid),
            LimitOrderId::Limit(uid) => Some(*uid),
            LimitOrderId::Liquidity(order) => match order {
                LiquidityOrderId::Protocol(uid) => Some(*uid),
                LiquidityOrderId::ZeroEx(_) => None,
            },
        }
    }
}

#[cfg(test)]
impl From<u32> for LimitOrderId {
    fn from(uid: u32) -> Self {
        Self::Market(OrderUid::from_integer(uid))
    }
}

/// Basic limit sell and buy orders
#[derive(Clone)]
#[cfg_attr(test, derive(Derivative))]
#[cfg_attr(test, derivative(PartialEq))]
pub struct LimitOrder {
    // Opaque Identifier for debugging purposes
    pub id: LimitOrderId,
    pub sell_token: H160,
    pub buy_token: H160,
    /// The amount that can be sold to acquire the required `buy_token`.
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub kind: OrderKind,
    pub partially_fillable: bool,
    /// The fee that should be used for objective value computations.
    /// Takes partiall fill into account.
    pub solver_fee: U256,
    #[cfg_attr(test, derivative(PartialEq = "ignore"))]
    pub settlement_handling: Arc<dyn SettlementHandling<Self>>,
    pub exchange: Exchange,
}

impl LimitOrder {
    pub fn is_liquidity_order(&self) -> bool {
        matches!(self.id, LimitOrderId::Liquidity(_))
    }

    /// For some orders the protocol doesn't precompute a fee. Instead solvers
    /// are supposed to compute a reasonable fee themselves.
    pub fn solver_determines_fee(&self) -> bool {
        self.partially_fillable && matches!(self.id, LimitOrderId::Limit(_))
    }
}

impl std::fmt::Debug for LimitOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Limit Order {:?}", self.id)
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LimitOrderExecution {
    /// The amount that the order `side` (`buy`, `sell`) should be filled by
    /// this trade.
    pub filled: U256,
    /// The fee (for the objective value) associated with this order.
    /// For partially fillable limit orders this value gets computed by the
    /// solver already refers to the `filled` amount. In this case no
    /// further scaling is necessary for partial fills. For all other orders
    /// this is the `solver_fee` for the entire order and will get scaled
    /// correctly by the [`SettlementEncoder`].
    pub solver_fee: U256,
}

impl LimitOrderExecution {
    pub fn new(filled: U256, solver_fee: U256) -> Self {
        Self { filled, solver_fee }
    }
}

impl Settleable for LimitOrder {
    type Execution = LimitOrderExecution;

    fn settlement_handling(&self) -> &dyn SettlementHandling<Self> {
        &*self.settlement_handling
    }
}

#[cfg(test)]
impl From<Order> for LimitOrder {
    fn from(order: Order) -> Self {
        order_converter::OrderConverter::test(H160([0x42; 20]))
            .normalize_limit_order(crate::order_balance_filter::BalancedOrder::full(order))
            .unwrap()
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
            solver_fee: Default::default(),
            settlement_handling: tests::CapturingSettlementHandler::arc(),
            id: Default::default(),
            exchange: Exchange::GnosisProtocol,
        }
    }
}

/// 2 sided constant product automated market maker with equal reserve value and
/// a trading fee (e.g. Uniswap, Sushiswap)
#[derive(Clone)]
#[cfg_attr(test, derive(Derivative))]
#[cfg_attr(test, derivative(PartialEq))]
pub struct ConstantProductOrder {
    pub address: H160,
    pub tokens: TokenPair,
    pub reserves: (u128, u128),
    pub fee: Ratio<u32>,
    #[cfg_attr(test, derivative(PartialEq = "ignore"))]
    pub settlement_handling: Arc<dyn SettlementHandling<Self>>,
}

impl ConstantProductOrder {
    /// Creates a new constant product order from a Uniswap V2 pool and a
    /// settlement handler implementation.
    pub fn for_pool(pool: Pool, settlement_handling: Arc<dyn SettlementHandling<Self>>) -> Self {
        Self {
            address: pool.address,
            tokens: pool.tokens,
            reserves: pool.reserves,
            fee: pool.fee,
            settlement_handling,
        }
    }
}

impl std::fmt::Debug for ConstantProductOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Constant Product AMM {:?}", self.tokens)
    }
}

#[cfg(test)]
impl From<Pool> for ConstantProductOrder {
    fn from(pool: Pool) -> Self {
        Self::for_pool(pool, tests::CapturingSettlementHandler::arc())
    }
}

/// 2 sided weighted product automated market maker with weighted reserves and a
/// trading fee (e.g. BalancerV2)
#[derive(Clone)]
#[cfg_attr(test, derive(Derivative))]
#[cfg_attr(test, derivative(PartialEq))]
pub struct WeightedProductOrder {
    pub address: H160,
    pub reserves: HashMap<H160, WeightedTokenState>,
    pub fee: Bfp,
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
    pub address: H160,
    pub reserves: HashMap<H160, TokenState>,
    pub fee: Bfp,
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
    pub input_max: TokenAmount,
    pub output: TokenAmount,
    pub internalizable: bool,
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

/// Concentrated type of liquidity with ticks (e.g. UniswapV3)
#[derive(Clone)]
#[cfg_attr(test, derive(Derivative))]
#[cfg_attr(test, derivative(PartialEq))]
pub struct ConcentratedLiquidity {
    pub tokens: TokenPair,
    pub pool: PoolInfo,
    #[cfg_attr(test, derivative(PartialEq = "ignore"))]
    pub settlement_handling: Arc<dyn SettlementHandling<Self>>,
}

impl std::fmt::Debug for ConcentratedLiquidity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Concentrated liquidity {:?}", self.pool)
    }
}

impl Settleable for ConcentratedLiquidity {
    type Execution = AmmOrderExecution;

    fn settlement_handling(&self) -> &dyn SettlementHandling<Self> {
        &*self.settlement_handling
    }
}

#[cfg(test)]
impl Default for ConstantProductOrder {
    fn default() -> Self {
        ConstantProductOrder {
            address: Default::default(),
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
            address: Default::default(),
            reserves: Default::default(),
            fee: Bfp::zero(),
            settlement_handling: tests::CapturingSettlementHandler::arc(),
        }
    }
}

#[cfg(test)]
impl Default for StablePoolOrder {
    fn default() -> Self {
        StablePoolOrder {
            address: Default::default(),
            reserves: Default::default(),
            fee: Default::default(),
            amplification_parameter: AmplificationParameter::new(1.into(), 1.into()).unwrap(),
            settlement_handling: tests::CapturingSettlementHandler::arc(),
        }
    }
}

#[cfg(test)]
pub mod tests {
    use {super::*, maplit::hashmap, std::sync::Mutex};

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
        L: Settleable + 'static,
        L::Execution: Send + Sync,
    {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

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
                sell_amount: sell_amount.into(),
                buy_amount: buy_amount.into(),
                kind,
                ..Default::default()
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
