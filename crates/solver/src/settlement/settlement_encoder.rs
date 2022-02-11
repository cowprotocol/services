use super::{Interaction, Trade, TradeExecution};
use crate::{encoding::EncodedSettlement, interactions::UnwrapWethInteraction};
use anyhow::{bail, ensure, Context as _, Result};
use model::order::{Order, OrderKind};
use num::{BigRational, One, Zero};
use primitive_types::{H160, U256};
use shared::conversions::{big_rational_to_u256, U256Ext};
use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    iter,
    sync::Arc,
};

/// An intermediate settlement representation that can be incrementally
/// constructed.
///
/// This allows liquidity to to encode itself into the settlement, in a way that
/// is completely decoupled from solvers, or how the liquidity is modelled.
/// Additionally, the fact that the settlement is kept in an intermediate
/// representation allows the encoder to potentially perform gas optimizations
/// (e.g. collapsing two interactions into one equivalent one).
#[derive(Debug, Clone)]
pub struct SettlementEncoder {
    // Make sure to update the `merge` method when adding new fields.

    // Invariant: tokens is all keys in clearing_prices sorted.
    tokens: Vec<H160>,
    clearing_prices: HashMap<H160, U256>,
    // Invariant: Every trade's buy and sell token has an entry in clearing_prices.
    trades: Vec<Trade>,
    // This is an Arc so that this struct is Clone. Cannot require `Interaction: Clone` because it
    // would make the trait not be object safe which prevents using it through `dyn`.
    // TODO: Can we fix this in a better way?
    execution_plan: Vec<Arc<dyn Interaction>>,
    unwraps: Vec<UnwrapWethInteraction>,
}

impl Default for SettlementEncoder {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl SettlementEncoder {
    /// Creates a new settlement encoder with the specified prices.
    ///
    /// The prices must be provided up front in order to ensure that all tokens
    /// included in the settlement are known when encoding trades.
    pub fn new(clearing_prices: HashMap<H160, U256>) -> Self {
        // Explicitly define a token ordering based on the supplied clearing
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

    #[cfg(test)]
    pub fn with_trades(clearing_prices: HashMap<H160, U256>, trades: Vec<Trade>) -> Self {
        let mut result = Self::new(clearing_prices);
        result.trades = trades;
        result
    }

    // Returns a copy of self without any liquidity provision interaction.
    pub fn without_onchain_liquidity(&self) -> Self {
        SettlementEncoder {
            tokens: self.tokens.clone(),
            clearing_prices: self.clearing_prices.clone(),
            trades: self.trades.clone(),
            execution_plan: Vec::new(),
            unwraps: self.unwraps.clone(),
        }
    }

    pub fn clearing_prices(&self) -> &HashMap<H160, U256> {
        &self.clearing_prices
    }

    pub fn trades(&self) -> &[Trade] {
        &self.trades
    }

    pub fn execution_plan(&self) -> &Vec<Arc<dyn Interaction>> {
        &self.execution_plan
    }

    // Fails if any used token doesn't have a price.
    pub fn add_trade(
        &mut self,
        order: Order,
        executed_amount: U256,
        scaled_unsubsidized_fee: U256,
        is_liquidity_order: bool,
    ) -> Result<TradeExecution> {
        let sell_price = self
            .clearing_prices
            .get(&order.order_creation.sell_token)
            .context("settlement missing sell token")?;
        let sell_token_index = self
            .token_index(order.order_creation.sell_token)
            .expect("missing sell token with price");

        let buy_price = self
            .clearing_prices
            .get(&order.order_creation.buy_token)
            .context("settlement missing buy token")?;
        let buy_token_index = self
            .token_index(order.order_creation.buy_token)
            .expect("missing buy token with price");

        let trade = Trade {
            order,
            sell_token_index,
            buy_token_index,
            executed_amount,
            scaled_unsubsidized_fee,
            is_liquidity_order,
        };
        let execution = trade
            .executed_amounts(*sell_price, *buy_price)
            .context("impossible trade execution")?;

        self.trades.push(trade);
        Ok(execution)
    }

    pub fn append_to_execution_plan(&mut self, interaction: impl Interaction + 'static) {
        self.execution_plan.push(Arc::new(interaction));
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

    pub fn add_token_equivalency(&mut self, token_a: H160, token_b: H160) -> Result<()> {
        let (new_token, existing_price) = match (
            self.clearing_prices.get(&token_a),
            self.clearing_prices.get(&token_b),
        ) {
            (Some(price_a), Some(price_b)) => {
                ensure!(
                    price_a == price_b,
                    "non-matching prices for equivalent tokens"
                );
                // Nothing to do, since both tokens are part of the solution and
                // have the same price (i.e. are equivalent).
                return Ok(());
            }
            (None, None) => bail!("tokens not part of solution for equivalency"),
            (Some(price_a), None) => (token_b, *price_a),
            (None, Some(price_b)) => (token_a, *price_b),
        };

        self.clearing_prices.insert(new_token, existing_price);
        self.tokens.push(new_token);

        // Now the tokens array is no longer sorted, so fix that, and make sure
        // to re-compute trade token indices as they may have changed.
        self.sort_tokens_and_update_indices();

        Ok(())
    }

    // Sort self.tokens and update all token indices in self.trades.
    fn sort_tokens_and_update_indices(&mut self) {
        self.tokens.sort();
        for i in 0..self.trades.len() {
            self.trades[i].sell_token_index = self
                .token_index(self.trades[i].order.order_creation.sell_token)
                .expect("missing sell token for existing trade");
            self.trades[i].buy_token_index = self
                .token_index(self.trades[i].order.order_creation.buy_token)
                .expect("missing buy token for existing trade");
        }
    }

    fn token_index(&self, token: H160) -> Option<usize> {
        self.tokens.binary_search(&token).ok()
    }

    pub fn total_surplus(
        &self,
        normalizing_prices: &HashMap<H160, BigRational>,
    ) -> Option<BigRational> {
        self.trades
            .iter()
            .filter(|trade| !trade.is_liquidity_order)
            .fold(Some(num::zero()), |acc, trade| {
                let order = trade.order.clone();
                let sell_token_clearing_price = self
                    .clearing_prices
                    .get(&order.order_creation.sell_token)
                    .expect("Solution with trade but without price for sell token")
                    .to_big_rational();
                let buy_token_clearing_price = self
                    .clearing_prices
                    .get(&order.order_creation.buy_token)
                    .expect("Solution with trade but without price for buy token")
                    .to_big_rational();

                let sell_token_external_price = normalizing_prices
                    .get(&order.order_creation.sell_token)
                    .expect("Solution with trade but without price for sell token");
                let buy_token_external_price = normalizing_prices
                    .get(&order.order_creation.buy_token)
                    .expect("Solution with trade but without price for buy token");

                if match order.order_creation.kind {
                    OrderKind::Sell => &buy_token_clearing_price,
                    OrderKind::Buy => &sell_token_clearing_price,
                }
                .is_zero()
                {
                    return None;
                }

                let surplus =
                    &trade.surplus(&sell_token_clearing_price, &buy_token_clearing_price)?;
                let normalized_surplus = match order.order_creation.kind {
                    OrderKind::Sell => {
                        surplus * buy_token_external_price / buy_token_clearing_price
                    }
                    OrderKind::Buy => {
                        surplus * sell_token_external_price / sell_token_clearing_price
                    }
                };
                Some(acc? + normalized_surplus)
            })
    }

    fn drop_unnecessary_tokens_and_prices(&mut self) {
        let traded_tokens: HashSet<_> = self
            .trades()
            .iter()
            .flat_map(|trade| {
                [
                    trade.order.order_creation.buy_token,
                    trade.order.order_creation.sell_token,
                ]
            })
            .collect();

        self.tokens.retain(|token| traded_tokens.contains(token));
        self.sort_tokens_and_update_indices();
        self.clearing_prices
            .retain(|token, _price| traded_tokens.contains(token));
    }

    pub fn finish(mut self) -> EncodedSettlement {
        self.drop_unnecessary_tokens_and_prices();

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

    // Merge other into self so that the result contains both settlements.
    // Fails if the settlements cannot be merged for example because the same limit order is used in
    // both or more than one token has a different clearing prices (a single token difference is scaled)
    pub fn merge(mut self, mut other: Self) -> Result<Self> {
        let scaling_factor = self.price_scaling_factor(&other);
        // Make sure we always scale prices up to avoid precision issues
        if scaling_factor < BigRational::one() {
            return other.merge(self);
        }
        for (key, value) in other.clearing_prices {
            let scaled_price = big_rational_to_u256(&(value.to_big_rational() * &scaling_factor))
                .context("Invalid price scaling factor")?;
            match self.clearing_prices.entry(key) {
                Entry::Occupied(entry) => ensure!(
                    *entry.get() == scaled_price,
                    "different price after scaling"
                ),
                Entry::Vacant(entry) => {
                    entry.insert(scaled_price);
                    self.tokens.push(key);
                }
            }
        }

        for other_trade in other.trades.iter() {
            ensure!(
                self.trades
                    .iter()
                    .all(|self_trade| self_trade.order.order_meta_data.uid
                        != other_trade.order.order_meta_data.uid),
                "duplicate trade"
            );
        }
        self.trades.append(&mut other.trades);
        self.sort_tokens_and_update_indices();

        self.execution_plan.append(&mut other.execution_plan);

        for unwrap in other.unwraps {
            self.add_unwrap(unwrap);
        }

        Ok(self)
    }

    fn price_scaling_factor(&self, other: &Self) -> BigRational {
        let self_keys: HashSet<_> = self.clearing_prices().keys().collect();
        let other_keys: HashSet<_> = other.clearing_prices().keys().collect();
        let common_tokens: Vec<_> = self_keys.intersection(&other_keys).collect();
        match common_tokens.first() {
            Some(token) => {
                let price_in_self = self
                    .clearing_prices
                    .get(token)
                    .expect("common token should be present")
                    .to_big_rational();
                let price_in_other = other
                    .clearing_prices
                    .get(token)
                    .expect("common token should be present")
                    .to_big_rational();
                price_in_self / price_in_other
            }
            None => U256::one().to_big_rational(),
        }
    }

    /// Drops all UnwrapWethInteractions for the given token address.
    /// This can be used in case the settlement contracts ETH buffer is big enough.
    pub fn drop_unwrap(&mut self, token: H160) {
        self.unwraps.retain(|unwrap| unwrap.weth.address() != token);
    }

    /// Calculates how much of a given token this settlement will unwrap during the execution.
    pub fn amount_to_unwrap(&self, token: H160) -> U256 {
        self.unwraps.iter().fold(U256::zero(), |sum, unwrap| {
            if unwrap.weth.address() == token {
                sum.checked_add(unwrap.amount)
                    .expect("no settlement would pay out that much ETH at once")
            } else {
                sum
            }
        })
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{encoding::EncodedInteraction, settlement::NoopInteraction};
    use contracts::WETH9;
    use ethcontract::Bytes;
    use maplit::hashmap;
    use model::order::{OrderBuilder, OrderCreation};
    use shared::dummy_contract;

    #[test]
    pub fn encode_trades_finds_token_index() {
        let token0 = H160::from_low_u64_be(0);
        let token1 = H160::from_low_u64_be(1);
        let order0 = Order {
            order_creation: OrderCreation {
                sell_token: token0,
                sell_amount: 1.into(),
                buy_token: token1,
                buy_amount: 1.into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let order1 = Order {
            order_creation: OrderCreation {
                sell_token: token1,
                sell_amount: 1.into(),
                buy_token: token0,
                buy_amount: 1.into(),
                ..Default::default()
            },
            ..Default::default()
        };

        let mut settlement = SettlementEncoder::new(maplit::hashmap! {
            token0 => 1.into(),
            token1 => 1.into(),
        });

        assert!(settlement
            .add_trade(order0, 1.into(), 1.into(), false)
            .is_ok());
        assert!(settlement
            .add_trade(order1, 1.into(), 0.into(), false)
            .is_ok());
    }

    #[test]
    fn settlement_merges_unwraps_for_same_token() {
        let weth = dummy_contract!(WETH9, [0x42; 20]);

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
            weth: dummy_contract!(WETH9, [0x01; 20]),
            amount: 1.into(),
        });
        encoder.add_unwrap(UnwrapWethInteraction {
            weth: dummy_contract!(WETH9, [0x02; 20]),
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
        let interaction: EncodedInteraction = (H160([0x01; 20]), 0.into(), Bytes(Vec::new()));
        let unwrap = UnwrapWethInteraction {
            weth: dummy_contract!(WETH9, [0x01; 20]),
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

    #[test]
    fn settlement_encoder_add_token_equivalency() {
        let token_a = H160([0x00; 20]);
        let token_b = H160([0xff; 20]);
        let mut encoder = SettlementEncoder::new(hashmap! {
            token_a => 1.into(),
            token_b => 2.into(),
        });
        encoder
            .add_trade(
                Order {
                    order_creation: OrderCreation {
                        sell_token: token_a,
                        sell_amount: 6.into(),
                        buy_token: token_b,
                        buy_amount: 3.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                0.into(),
                0.into(),
                false,
            )
            .unwrap();

        assert_eq!(encoder.tokens, [token_a, token_b]);
        assert_eq!(encoder.trades[0].sell_token_index, 0);
        assert_eq!(encoder.trades[0].buy_token_index, 1);

        let token_c = H160([0xee; 20]);
        encoder.add_token_equivalency(token_a, token_c).unwrap();

        assert_eq!(encoder.tokens, [token_a, token_c, token_b]);
        assert_eq!(
            encoder.clearing_prices[&token_a],
            encoder.clearing_prices[&token_c],
        );
        assert_eq!(encoder.trades[0].sell_token_index, 0);
        assert_eq!(encoder.trades[0].buy_token_index, 2);
    }

    #[test]
    fn settlement_encoder_token_equivalency_missing_tokens() {
        let mut encoder = SettlementEncoder::new(HashMap::new());
        assert!(encoder
            .add_token_equivalency(H160([0; 20]), H160([1; 20]))
            .is_err());
    }

    #[test]
    fn settlement_encoder_non_equivalent_tokens() {
        let token_a = H160([1; 20]);
        let token_b = H160([2; 20]);
        let mut encoder = SettlementEncoder::new(hashmap! {
            token_a => 1.into(),
            token_b => 2.into(),
        });
        assert!(encoder.add_token_equivalency(token_a, token_b).is_err());
    }

    fn token(number: u64) -> H160 {
        H160::from_low_u64_be(number)
    }

    #[test]
    fn merge_ok() {
        let weth = dummy_contract!(WETH9, H160::zero());

        let prices = hashmap! { token(1) => 1.into(), token(3) => 3.into() };
        let mut encoder0 = SettlementEncoder::new(prices);
        let mut order13 = OrderBuilder::default()
            .with_sell_token(token(1))
            .with_sell_amount(33.into())
            .with_buy_token(token(3))
            .with_buy_amount(11.into())
            .build();
        order13.order_meta_data.uid.0[0] = 0;
        encoder0
            .add_trade(order13, 13.into(), 0.into(), false)
            .unwrap();
        encoder0.append_to_execution_plan(NoopInteraction {});
        encoder0.add_unwrap(UnwrapWethInteraction {
            weth: weth.clone(),
            amount: 1.into(),
        });

        let prices = hashmap! { token(2) => 2.into(), token(4) => 4.into() };
        let mut encoder1 = SettlementEncoder::new(prices);
        let mut order24 = OrderBuilder::default()
            .with_sell_token(token(2))
            .with_sell_amount(44.into())
            .with_buy_token(token(4))
            .with_buy_amount(22.into())
            .build();
        order24.order_meta_data.uid.0[0] = 1;
        encoder1
            .add_trade(order24, 24.into(), 0.into(), false)
            .unwrap();
        encoder1.append_to_execution_plan(NoopInteraction {});
        encoder1.add_unwrap(UnwrapWethInteraction {
            weth,
            amount: 2.into(),
        });

        let merged = encoder0.merge(encoder1).unwrap();
        let prices = hashmap! {
            token(1) => 1.into(), token(3) => 3.into(),
            token(2) => 2.into(), token(4) => 4.into(),
        };
        assert_eq!(merged.clearing_prices, prices);
        assert_eq!(merged.tokens, [token(1), token(2), token(3), token(4)]);
        assert_eq!(merged.trades.len(), 2);
        assert_eq!(merged.execution_plan.len(), 2);
        assert_eq!(merged.unwraps[0].amount, 3.into());
    }

    #[test]
    fn merge_fails_because_price_is_different() {
        let prices = hashmap! { token(1) => 1.into(), token(2) => 2.into() };
        let encoder0 = SettlementEncoder::new(prices);
        let prices = hashmap! { token(1) => 1.into(), token(2) => 4.into() };
        let encoder1 = SettlementEncoder::new(prices);
        assert!(encoder0.merge(encoder1).is_err());
    }

    #[test]
    fn merge_scales_prices_if_only_one_token_used_twice() {
        let prices = hashmap! { token(1) => 2.into(), token(2) => 2.into() };
        let encoder0 = SettlementEncoder::new(prices);
        let prices = hashmap! { token(1) => 1.into(), token(3) => 3.into() };
        let encoder1 = SettlementEncoder::new(prices);

        let merged = encoder0.merge(encoder1).unwrap();
        let prices = hashmap! {
            token(1) => 2.into(),
            token(2) => 2.into(),
            token(3) => 6.into(),
        };
        assert_eq!(merged.clearing_prices, prices);
    }

    #[test]
    fn merge_always_scales_smaller_price_up() {
        let prices = hashmap! { token(1) => 1.into(), token(2) => 1_000_000.into() };
        let encoder0 = SettlementEncoder::new(prices);
        let prices = hashmap! { token(1) => 1_000_000.into(), token(3) => 900_000.into() };
        let encoder1 = SettlementEncoder::new(prices);

        let merge01 = encoder0.clone().merge(encoder1.clone()).unwrap();
        let merge10 = encoder1.merge(encoder0).unwrap();
        assert_eq!(merge10.clearing_prices, merge01.clearing_prices);

        // If scaled down 900k would have become 0
        assert_eq!(
            *merge10.clearing_prices.get(&token(3)).unwrap(),
            900_000.into()
        );
    }

    #[test]
    fn merge_fails_because_trade_used_twice() {
        let prices = hashmap! { token(1) => 1.into(), token(3) => 3.into() };
        let order13 = OrderBuilder::default()
            .with_sell_token(token(1))
            .with_sell_amount(33.into())
            .with_buy_token(token(3))
            .with_buy_amount(11.into())
            .build();

        let mut encoder0 = SettlementEncoder::new(prices.clone());
        encoder0
            .add_trade(order13.clone(), 13.into(), 0.into(), false)
            .unwrap();

        let mut encoder1 = SettlementEncoder::new(prices);
        encoder1
            .add_trade(order13, 24.into(), 0.into(), false)
            .unwrap();

        assert!(encoder0.merge(encoder1).is_err());
    }

    #[test]
    fn encoding_strips_unnecessary_tokens_and_prices() {
        let prices = hashmap! {token(1) => 7.into(), token(2) => 2.into(),
        token(3) => 9.into(), token(4) => 44.into()};

        let mut encoder = SettlementEncoder::new(prices);

        let order_1_3 = OrderBuilder::default()
            .with_sell_token(token(1))
            .with_sell_amount(33.into())
            .with_buy_token(token(3))
            .with_buy_amount(11.into())
            .build();
        encoder
            .add_trade(order_1_3, 4.into(), 0.into(), false)
            .unwrap();

        let weth = dummy_contract!(WETH9, token(2));
        encoder.add_unwrap(UnwrapWethInteraction {
            weth,
            amount: 12.into(),
        });

        let encoded = encoder.finish();

        // only token 1 and 2 have been included in orders by traders
        let expected_tokens: Vec<_> = [1, 3].into_iter().map(token).collect();
        assert_eq!(expected_tokens, encoded.tokens);

        // only the prices for token 1 and 2 remain and they are in the correct order
        let expected_prices: Vec<_> = [7, 9].into_iter().map(U256::from).collect();
        assert_eq!(expected_prices, encoded.clearing_prices);

        let encoded_trade = &encoded.trades[0];

        // dropping unnecessary tokens did not change the sell_token_index
        let updated_sell_token_index = encoded_trade.0;
        assert_eq!(updated_sell_token_index, 0.into());

        // dropping unnecessary tokens decreased the buy_token_index by one
        let updated_buy_token_index = encoded_trade.1;
        assert_eq!(updated_buy_token_index, 1.into());
    }
}
