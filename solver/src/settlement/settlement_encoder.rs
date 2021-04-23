use super::{Interaction, Trade};
use crate::{encoding::EncodedSettlement, interactions::UnwrapWethInteraction};
use anyhow::{anyhow, bail, ensure, Result};
use model::order::{Order, OrderKind};
use num::{BigRational, Zero};
use primitive_types::{H160, U256};
use shared::conversions::U256Ext;
use std::{collections::HashMap, iter};

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
    pub fn new(clearing_prices: HashMap<H160, U256>) -> Self {
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

    #[cfg(test)]
    pub fn with_trades(clearing_prices: HashMap<H160, U256>, trades: Vec<Trade>) -> Self {
        let mut result = Self::new(clearing_prices);
        result.trades = trades;
        result
    }

    pub fn clearing_prices(&self) -> &HashMap<H160, U256> {
        &self.clearing_prices
    }

    pub fn trades(&self) -> &[Trade] {
        &self.trades
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
        self.tokens.sort();
        for i in 0..self.trades.len() {
            self.trades[i].sell_token_index = self
                .token_index(self.trades[i].order.order_creation.sell_token)
                .expect("missing sell token for exisiting trade");
            self.trades[i].buy_token_index = self
                .token_index(self.trades[i].order.order_creation.buy_token)
                .expect("missing buy token for exisiting trade");
        }

        Ok(())
    }

    fn token_index(&self, token: H160) -> Option<usize> {
        self.tokens.binary_search(&token).ok()
    }

    pub fn total_surplus(
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

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{encoding::EncodedInteraction, interactions::dummy_web3};
    use maplit::hashmap;
    use model::order::OrderCreation;

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
                        buy_token: token_b,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                0.into(),
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
}
