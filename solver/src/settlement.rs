use crate::{
    encoding::{self, EncodedInteraction, EncodedSettlement, EncodedTrade},
    interactions::UnwrapWethInteraction,
    liquidity::Settleable,
};
use anyhow::{anyhow, Result};
use model::order::{Order, OrderKind};
use num::{BigRational, Signed, Zero};
use primitive_types::{H160, U256};
use shared::conversions::U256Ext;
use std::{collections::HashMap, iter};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Trade {
    pub order: Order,
    pub sell_token_index: usize,
    pub buy_token_index: usize,
    pub executed_amount: U256,
}

impl Trade {
    // The difference between the minimum you were willing to buy/maximum you were willing to sell, and what you ended up buying/selling
    pub fn surplus(
        &self,
        sell_token_price: &BigRational,
        buy_token_price: &BigRational,
    ) -> Option<BigRational> {
        match self.order.order_creation.kind {
            model::order::OrderKind::Buy => buy_order_surplus(
                sell_token_price,
                buy_token_price,
                &self.order.order_creation.sell_amount.to_big_rational(),
                &self.order.order_creation.buy_amount.to_big_rational(),
                &self.executed_amount.to_big_rational(),
            ),
            model::order::OrderKind::Sell => sell_order_surplus(
                sell_token_price,
                buy_token_price,
                &self.order.order_creation.sell_amount.to_big_rational(),
                &self.order.order_creation.buy_amount.to_big_rational(),
                &self.executed_amount.to_big_rational(),
            ),
        }
    }

    /// Encodes the settlement trade as a tuple, as expected by the smart
    /// contract.
    pub fn encode(&self) -> EncodedTrade {
        encoding::encode_trade(
            &self.order.order_creation,
            self.sell_token_index,
            self.buy_token_index,
            &self.executed_amount,
        )
    }
}

pub trait Interaction: std::fmt::Debug + Send {
    // TODO: not sure if this should return a result.
    // Write::write returns a result but we know we write to a vector in memory so we know it will
    // never fail. Then the question becomes whether interactions should be allowed to fail encoding
    // for other reasons.
    fn encode(&self) -> Vec<EncodedInteraction>;
}

#[cfg(test)]
impl Interaction for EncodedInteraction {
    fn encode(&self) -> Vec<EncodedInteraction> {
        vec![self.clone()]
    }
}

/// An intermediate settlement representation that can be incrementally
/// constructed.
///
/// This allows liquidity to to encode itself into the settlement, in a way that
/// is completely decoupled from solvers, or how the liquidity is modelled.
/// Additionally, the fact that the settlement is kept in an intermediate
/// representation allows the encoder to potentially perform gas optimizations
/// (e.g. collapsing two interactions into one equivalent one).
#[derive(Debug)]
pub struct SettlementEncoder {
    tokens: Vec<H160>,
    clearing_prices: HashMap<H160, U256>,
    trades: Vec<Trade>,
    execution_plan: Vec<Box<dyn Interaction>>,
    unwraps: Vec<UnwrapWethInteraction>,
}

impl SettlementEncoder {
    /// Creates a new settlement encoder with the specified prices.
    ///
    /// The prices must be provided up front in order to ensure that all tokens
    /// included in the settlement are known when encoding trades.
    fn new(clearing_prices: HashMap<H160, U256>) -> Self {
        // Explicitely define a token ordering based on the supplied clearing
        // prices. This is done since `HashMap::keys` returns an iterator in
        // arbitrary order ([1]), meaning that we can't rely that the ordering
        // will be consistent across calls. The list is sorted so that
        // settlements with the same encoded trades and interactions produce
        // the same resulting encoded settlement, and so that we can use binary
        // searching in order to find token indices.
        // [1]: https://doc.rust-lang.org/beta/std/collections/hash_map/struct.HashMap.html#method.keys
        let mut tokens = clearing_prices.keys().copied().collect::<Vec<_>>();
        tokens.sort();

        SettlementEncoder {
            tokens,
            clearing_prices,
            trades: Vec::new(),
            execution_plan: Vec::new(),
            unwraps: Vec::new(),
        }
    }

    pub fn add_trade(&mut self, order: Order, executed_amount: U256) -> Result<()> {
        let sell_token_index = self
            .token_index(order.order_creation.sell_token)
            .ok_or_else(|| anyhow!("settlement missing sell token"))?;
        let buy_token_index = self
            .token_index(order.order_creation.buy_token)
            .ok_or_else(|| anyhow!("settlement missing buy token"))?;
        self.trades.push(Trade {
            order,
            sell_token_index,
            buy_token_index,
            executed_amount,
        });

        Ok(())
    }

    pub fn append_to_execution_plan(&mut self, interaction: impl Interaction + 'static) {
        self.execution_plan.push(Box::new(interaction));
    }

    pub fn add_unwrap(&mut self, unwrap: UnwrapWethInteraction) {
        for existing_unwrap in self.unwraps.iter_mut() {
            if existing_unwrap.merge(&unwrap).is_ok() {
                return;
            }
        }

        // If the native token unwrap can't be merged with any existing ones,
        // just add it to the vector.
        self.unwraps.push(unwrap);
    }

    fn token_index(&self, token: H160) -> Option<usize> {
        self.tokens.binary_search(&token).ok()
    }

    fn total_surplus(
        &self,
        normalizing_prices: &HashMap<H160, BigRational>,
    ) -> Option<BigRational> {
        self.trades.iter().fold(Some(num::zero()), |acc, trade| {
            let sell_token_clearing_price = self
                .clearing_prices
                .get(&trade.order.order_creation.sell_token)
                .expect("Solution with trade but without price for sell token")
                .to_big_rational();
            let buy_token_clearing_price = self
                .clearing_prices
                .get(&trade.order.order_creation.buy_token)
                .expect("Solution with trade but without price for buy token")
                .to_big_rational();

            let sell_token_external_price = normalizing_prices
                .get(&trade.order.order_creation.sell_token)
                .expect("Solution with trade but without price for sell token");
            let buy_token_external_price = normalizing_prices
                .get(&trade.order.order_creation.buy_token)
                .expect("Solution with trade but without price for buy token");

            if match trade.order.order_creation.kind {
                OrderKind::Sell => &buy_token_clearing_price,
                OrderKind::Buy => &sell_token_clearing_price,
            }
            .is_zero()
            {
                return None;
            }

            let surplus = &trade.surplus(&sell_token_clearing_price, &buy_token_clearing_price)?;
            let normalized_surplus = match trade.order.order_creation.kind {
                OrderKind::Sell => surplus * buy_token_external_price / buy_token_clearing_price,
                OrderKind::Buy => surplus * sell_token_external_price / sell_token_clearing_price,
            };
            Some(acc? + normalized_surplus)
        })
    }

    pub fn finish(self) -> EncodedSettlement {
        let clearing_prices = self
            .tokens
            .iter()
            .map(|token| {
                *self
                    .clearing_prices
                    .get(token)
                    .expect("missing clearing price for token")
            })
            .collect();

        EncodedSettlement {
            tokens: self.tokens,
            clearing_prices,
            trades: self
                .trades
                .into_iter()
                .map(|trade| trade.encode())
                .collect(),
            interactions: [
                Vec::new(),
                iter::empty()
                    .chain(
                        self.execution_plan
                            .iter()
                            .flat_map(|interaction| interaction.encode()),
                    )
                    .chain(self.unwraps.iter().flat_map(|unwrap| unwrap.encode()))
                    .collect(),
                Vec::new(),
            ],
        }
    }
}

#[derive(Debug)]
pub struct Settlement {
    encoder: SettlementEncoder,
}

impl Settlement {
    /// Creates a new settlement builder for the specified clearing prices.
    pub fn new(clearing_prices: HashMap<H160, U256>) -> Self {
        Self {
            encoder: SettlementEncoder::new(clearing_prices),
        }
    }

    /// .
    pub fn with_liquidity<L>(&mut self, liquidity: &L, execution: L::Execution) -> Result<()>
    where
        L: Settleable,
    {
        liquidity
            .settlement_handling()
            .encode(execution, &mut self.encoder)
    }

    /// Returns the clearing prices map.
    pub fn clearing_prices(&self) -> &HashMap<H160, U256> {
        &self.encoder.clearing_prices
    }

    /// Returns the clearing price for the specified token.
    ///
    /// Returns `None` if the token is not part of the settlement.
    pub fn clearing_price(&self, token: H160) -> Option<U256> {
        self.encoder.clearing_prices.get(&token).copied()
    }

    /// Returns the currently encoded trades.
    pub fn trades(&self) -> &[Trade] {
        &self.encoder.trades
    }

    // For now this computes the total surplus of all EOA trades.
    pub fn objective_value(&self, external_prices: &HashMap<H160, BigRational>) -> BigRational {
        match self.encoder.total_surplus(&external_prices) {
            Some(value) => value,
            None => {
                tracing::error!("Overflow computing objective value for: {:?}", self);
                num::zero()
            }
        }
    }
}

impl From<Settlement> for EncodedSettlement {
    fn from(settlement: Settlement) -> Self {
        settlement.encoder.finish()
    }
}

// The difference between what you were willing to sell (executed_amount * limit_price) converted into reference token (multiplied by buy_token_price)
// and what you had to sell denominated in the reference token (executed_amount * buy_token_price)
fn buy_order_surplus(
    sell_token_price: &BigRational,
    buy_token_price: &BigRational,
    sell_amount_limit: &BigRational,
    buy_amount_limit: &BigRational,
    executed_amount: &BigRational,
) -> Option<BigRational> {
    if buy_amount_limit.is_zero() {
        return None;
    }
    let res = executed_amount * sell_amount_limit / buy_amount_limit * sell_token_price
        - (executed_amount * buy_token_price);
    if res.is_negative() {
        None
    } else {
        Some(res)
    }
}

// The difference of your proceeds denominated in the reference token (executed_sell_amount * sell_token_price)
// and what you were minimally willing to receive in buy tokens (executed_sell_amount * limit_price)
// converted to amount in reference token at the effective price (multiplied by buy_token_price)
fn sell_order_surplus(
    sell_token_price: &BigRational,
    buy_token_price: &BigRational,
    sell_amount_limit: &BigRational,
    buy_amount_limit: &BigRational,
    executed_amount: &BigRational,
) -> Option<BigRational> {
    if sell_amount_limit.is_zero() {
        return None;
    }
    let res = executed_amount * sell_token_price
        - (executed_amount * buy_amount_limit / sell_amount_limit * buy_token_price);
    if res.is_negative() {
        None
    } else {
        Some(res)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interactions::dummy_web3;
    use model::order::{OrderCreation, OrderKind};
    use num::FromPrimitive;

    #[test]
    pub fn encode_trades_finds_token_index() {
        let token0 = H160::from_low_u64_be(0);
        let token1 = H160::from_low_u64_be(1);
        let order0 = Order {
            order_creation: OrderCreation {
                sell_token: token0,
                buy_token: token1,
                ..Default::default()
            },
            ..Default::default()
        };
        let order1 = Order {
            order_creation: OrderCreation {
                sell_token: token1,
                buy_token: token0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut settlement = SettlementEncoder::new(maplit::hashmap! {
            token0 => 0.into(),
            token1 => 0.into(),
        });

        assert!(settlement.add_trade(order0, 0.into()).is_ok());
        assert!(settlement.add_trade(order1, 0.into()).is_ok());
    }

    // Helper function to save some repeatition below.
    fn r(u: u128) -> BigRational {
        BigRational::from_u128(u).unwrap()
    }

    /// Helper function for creating a settlement for the specified prices and
    /// trades for testing objective value computations.
    fn test_settlement(prices: HashMap<H160, U256>, trades: Vec<Trade>) -> Settlement {
        Settlement {
            encoder: SettlementEncoder {
                trades,
                ..SettlementEncoder::new(prices)
            },
        }
    }

    #[test]
    pub fn objective_value() {
        let token0 = H160::from_low_u64_be(0);
        let token1 = H160::from_low_u64_be(1);

        let order0 = Order {
            order_creation: OrderCreation {
                sell_token: token0,
                buy_token: token1,
                sell_amount: 10.into(),
                buy_amount: 9.into(),
                kind: OrderKind::Sell,
                ..Default::default()
            },
            ..Default::default()
        };
        let order1 = Order {
            order_creation: OrderCreation {
                sell_token: token1,
                buy_token: token0,
                sell_amount: 10.into(),
                buy_amount: 9.into(),
                kind: OrderKind::Sell,
                ..Default::default()
            },
            ..Default::default()
        };

        let trade0 = Trade {
            order: order0.clone(),
            executed_amount: 10.into(),
            ..Default::default()
        };
        let trade1 = Trade {
            order: order1.clone(),
            executed_amount: 10.into(),
            ..Default::default()
        };

        // Case where external price vector doesn't influence ranking:

        let clearing_prices0 = maplit::hashmap! {token0 => 1.into(), token1 => 1.into()};
        let clearing_prices1 = maplit::hashmap! {token0 => 2.into(), token1 => 2.into()};

        let settlement0 = test_settlement(clearing_prices0, vec![trade0.clone(), trade1.clone()]);

        let settlement1 = test_settlement(clearing_prices1, vec![trade0, trade1]);

        let external_prices = maplit::hashmap! {token0 => r(1), token1 => r(1)};
        assert_eq!(
            settlement0.objective_value(&external_prices),
            settlement1.objective_value(&external_prices)
        );

        let external_prices = maplit::hashmap! {token0 => r(2), token1 => r(1)};
        assert_eq!(
            settlement0.objective_value(&external_prices),
            settlement1.objective_value(&external_prices)
        );

        // Case where external price vector influences ranking:

        let trade0 = Trade {
            order: order0.clone(),
            executed_amount: 10.into(),
            ..Default::default()
        };
        let trade1 = Trade {
            order: order1.clone(),
            executed_amount: 9.into(),
            ..Default::default()
        };

        let clearing_prices0 = maplit::hashmap! {token0 => 9.into(), token1 => 10.into()};

        // Settlement0 gets the following surpluses:
        // trade0: 81 - 81 = 0
        // trade1: 100 - 81 = 19
        let settlement0 = test_settlement(clearing_prices0, vec![trade0, trade1]);

        let trade0 = Trade {
            order: order0,
            executed_amount: 9.into(),
            ..Default::default()
        };
        let trade1 = Trade {
            order: order1,
            executed_amount: 10.into(),
            ..Default::default()
        };

        let clearing_prices1 = maplit::hashmap! {token0 => 10.into(), token1 => 9.into()};

        // Settlement1 gets the following surpluses:
        // trade0: 90 - 72.9 = 17.1
        // trade1: 100 - 100 = 0
        let settlement1 = test_settlement(clearing_prices1, vec![trade0, trade1]);

        // If the external prices of the two tokens is the same, then both settlements are symmetric.
        let external_prices = maplit::hashmap! {token0 => r(1), token1 => r(1)};
        assert_eq!(
            settlement0.objective_value(&external_prices),
            settlement1.objective_value(&external_prices)
        );

        // If the external price of the first token is higher, then the first settlement is preferred.
        let external_prices = maplit::hashmap! {token0 => r(2), token1 => r(1)};

        // Settlement0 gets the following normalized surpluses:
        // trade0: 0
        // trade1: 19 * 2 / 10 = 3.8

        // Settlement1 gets the following normalized surpluses:
        // trade0: 17.1 * 1 / 9 = 1.9
        // trade1: 0

        assert!(
            settlement0.objective_value(&external_prices)
                > settlement1.objective_value(&external_prices)
        );

        // If the external price of the second token is higher, then the second settlement is preferred.
        // (swaps above normalized surpluses of settlement0 and settlement1)
        let external_prices = maplit::hashmap! {token0 => r(1), token1 => r(2)};

        assert!(
            settlement0.objective_value(&external_prices)
                < settlement1.objective_value(&external_prices)
        );
    }

    #[test]
    fn test_computing_objective_value_with_zero_prices() {
        // Test if passing a clearing price of zero to the objective value function does
        // not panic.

        let token0 = H160::from_low_u64_be(0);
        let token1 = H160::from_low_u64_be(1);

        let order = Order {
            order_creation: OrderCreation {
                sell_token: token0,
                buy_token: token1,
                sell_amount: 10.into(),
                buy_amount: 9.into(),
                kind: OrderKind::Sell,
                ..Default::default()
            },
            ..Default::default()
        };

        let trade = Trade {
            order,
            executed_amount: 10.into(),
            ..Default::default()
        };

        let clearing_prices = maplit::hashmap! {token0 => 1.into(), token1 => 0.into()};

        let settlement = test_settlement(clearing_prices, vec![trade]);

        let external_prices = maplit::hashmap! {token0 => r(1), token1 => r(1)};
        settlement.objective_value(&external_prices);
    }

    #[test]
    #[allow(clippy::just_underscores_and_digits)]
    fn test_buy_order_surplus() {
        // Two goods are worth the same (100 each). If we were willing to pay up to 60 to receive 50,
        // but ended paying the price (1) we have a surplus of 10 sell units, so a total surplus of 1000.

        assert_eq!(
            buy_order_surplus(&r(100), &r(100), &r(60), &r(50), &r(50)),
            Some(r(1000))
        );

        // If our trade got only half filled, we only get half the surplus
        assert_eq!(
            buy_order_surplus(&r(100), &r(100), &r(60), &r(50), &r(25)),
            Some(r(500))
        );

        // No surplus if trade is not at all filled
        assert_eq!(
            buy_order_surplus(&r(100), &r(100), &r(60), &r(50), &r(0)),
            Some(r(0))
        );

        // No surplus if trade is filled at limit
        assert_eq!(
            buy_order_surplus(&r(100), &r(100), &r(50), &r(50), &r(50)),
            Some(r(0))
        );

        // Arithmetic error when limit price not respected
        assert_eq!(
            buy_order_surplus(&r(100), &r(100), &r(40), &r(50), &r(50)),
            None
        );

        // Sell Token worth twice as much as buy token. If we were willing to sell at parity, we will
        // have a surplus of 50% of tokens, worth 200 each.
        assert_eq!(
            buy_order_surplus(&r(200), &r(100), &r(50), &r(50), &r(50)),
            Some(r(5000))
        );

        // Buy Token worth twice as much as sell token. If we were willing to sell at 3:1, we will
        // have a surplus of 20 sell tokens, worth 100 each.
        assert_eq!(
            buy_order_surplus(&r(100), &r(200), &r(60), &r(20), &r(20)),
            Some(r(2000))
        );
    }

    #[test]
    #[allow(clippy::just_underscores_and_digits)]
    fn test_sell_order_surplus() {
        // Two goods are worth the same (100 each). If we were willing to receive as little as 40,
        // but ended paying the price (1) we have a surplus of 10 bought units, so a total surplus of 1000.

        assert_eq!(
            sell_order_surplus(&r(100), &r(100), &r(50), &r(40), &r(50)),
            Some(r(1000))
        );

        // If our trade got only half filled, we only get half the surplus
        assert_eq!(
            sell_order_surplus(&r(100), &r(100), &r(50), &r(40), &r(25)),
            Some(r(500))
        );

        // No surplus if trade is not at all filled
        assert_eq!(
            sell_order_surplus(&r(100), &r(100), &r(50), &r(40), &r(0)),
            Some(r(0))
        );

        // No surplus if trade is filled at limit
        assert_eq!(
            sell_order_surplus(&r(100), &r(100), &r(50), &r(50), &r(50)),
            Some(r(0))
        );

        // Arithmetic error when limit price not respected
        assert_eq!(
            sell_order_surplus(&r(100), &r(100), &r(50), &r(60), &r(50)),
            None
        );

        // Sell token worth twice as much as buy token. If we were willing to buy at parity, we will
        // have a surplus of 100% of buy tokens, worth 100 each.
        assert_eq!(
            sell_order_surplus(&r(200), &r(100), &r(50), &r(50), &r(50)),
            Some(r(5000))
        );

        // Buy Token worth twice as much as sell token. If we were willing to buy at 3:1, we will
        // have a surplus of 10 sell tokens, worth 200 each.
        assert_eq!(
            buy_order_surplus(&r(100), &r(200), &r(60), &r(20), &r(20)),
            Some(r(2000))
        );
    }

    #[test]
    fn settlement_merges_unwraps_for_same_token() {
        let weth = dummy_web3::dummy_weth([0x42; 20]);

        let mut encoder = SettlementEncoder::new(HashMap::new());
        encoder.add_unwrap(UnwrapWethInteraction {
            weth: weth.clone(),
            amount: 1.into(),
        });
        encoder.add_unwrap(UnwrapWethInteraction {
            weth: weth.clone(),
            amount: 2.into(),
        });

        assert_eq!(
            encoder.finish().interactions[1],
            UnwrapWethInteraction {
                weth,
                amount: 3.into(),
            }
            .encode(),
        );
    }

    #[test]
    fn settlement_encoder_appends_unwraps_for_different_tokens() {
        let mut encoder = SettlementEncoder::new(HashMap::new());
        encoder.add_unwrap(UnwrapWethInteraction {
            weth: dummy_web3::dummy_weth([0x01; 20]),
            amount: 1.into(),
        });
        encoder.add_unwrap(UnwrapWethInteraction {
            weth: dummy_web3::dummy_weth([0x02; 20]),
            amount: 2.into(),
        });

        assert_eq!(
            encoder
                .unwraps
                .iter()
                .map(|unwrap| (unwrap.weth.address().0, unwrap.amount.as_u64()))
                .collect::<Vec<_>>(),
            vec![([0x01; 20], 1), ([0x02; 20], 2)],
        );
    }

    #[test]
    fn settlement_unwraps_after_execution_plan() {
        let interaction: EncodedInteraction = (H160([0x01; 20]), 0.into(), Vec::new());
        let unwrap = UnwrapWethInteraction {
            weth: dummy_web3::dummy_weth([0x01; 20]),
            amount: 1.into(),
        };

        let mut encoder = SettlementEncoder::new(HashMap::new());
        encoder.add_unwrap(unwrap.clone());
        encoder.append_to_execution_plan(interaction.clone());

        assert_eq!(
            encoder.finish().interactions[1],
            [interaction.encode(), unwrap.encode()].concat(),
        );
    }
}
