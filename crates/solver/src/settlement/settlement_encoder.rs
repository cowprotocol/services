use {
    super::{trade_surplus_in_native_token_with_prices, ExternalPrices, Trade, TradeExecution},
    crate::{encoding::EncodedSettlement, interactions::UnwrapWethInteraction},
    anyhow::{bail, ensure, Context as _, Result},
    itertools::{Either, Itertools},
    model::{
        interaction::InteractionData,
        order::{LimitOrderClass, Order, OrderClass, OrderKind},
    },
    num::{BigRational, One},
    number_conversions::big_rational_to_u256,
    primitive_types::{H160, U256},
    shared::{
        conversions::U256Ext,
        http_solver::model::InternalizationStrategy,
        interaction::Interaction,
    },
    std::{
        collections::{hash_map::Entry, HashMap, HashSet},
        iter,
        sync::Arc,
    },
};

/// An interaction paired with a flag indicating whether it can be omitted
/// from the final execution plan
type MaybeInternalizableInteraction = (Arc<dyn Interaction>, bool);

/// An intermediate settlement representation that can be incrementally
/// constructed.
///
/// This allows liquidity to to encode itself into the settlement, in a way that
/// is completely decoupled from solvers, or how the liquidity is modelled.
/// Additionally, the fact that the settlement is kept in an intermediate
/// representation allows the encoder to potentially perform gas optimizations
/// (e.g. collapsing two interactions into one equivalent one).
#[derive(Debug, Clone, Default)]
pub struct SettlementEncoder {
    // Make sure to update the `merge` method when adding new fields.

    // Invariant: tokens is all keys in clearing_prices sorted.
    tokens: Vec<H160>,
    clearing_prices: HashMap<H160, U256>,
    trades: Vec<EncoderTrade>,
    // This is an Arc so that this struct is Clone. Cannot require `Interaction: Clone` because it
    // would make the trait not be object safe which prevents using it through `dyn`.
    // TODO: Can we fix this in a better way?
    execution_plan: Vec<MaybeInternalizableInteraction>,
    pre_interactions: Vec<InteractionData>,
    unwraps: Vec<UnwrapWethInteraction>,
}

/// References to the trade's tokens into the clearing price vector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TokenReference {
    Indexed {
        sell_token_index: usize,
        buy_token_index: usize,
    },

    /// Token reference for orders with a price that can be different from the
    /// uniform clearing price. This means that these prices need to be stored
    /// outside of the uniform clearing price vector.
    /// This is required for liquidity orders and limit orders.
    /// Liquidity orders are not allowed to get surplus and therefore
    /// have to be settled at their limit price. Prices for limit orders have to
    /// be adjusted slightly to account for the `surplus_fee` mark up.
    CustomPrice {
        sell_token_price: U256,
        buy_token_price: U256,
    },
}

impl Default for TokenReference {
    fn default() -> Self {
        Self::Indexed {
            sell_token_index: 0,
            buy_token_index: 0,
        }
    }
}

/// An trade that was added to the settlement encoder.
#[derive(Clone, Debug, Eq, PartialEq)]
struct EncoderTrade {
    data: Trade,
    tokens: TokenReference,
}

/// A trade with token prices.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PricedTrade<'a> {
    pub data: &'a Trade,
    pub sell_token_price: U256,
    pub buy_token_price: U256,
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
            pre_interactions: Vec::new(),
            unwraps: Vec::new(),
        }
    }

    #[cfg(test)]
    pub fn with_trades(clearing_prices: HashMap<H160, U256>, trades: Vec<Trade>) -> Self {
        let mut result = Self::new(clearing_prices);
        for trade in trades {
            result
                .add_trade(
                    trade.order,
                    trade.executed_amount,
                    trade.scaled_unsubsidized_fee,
                )
                .unwrap();
        }
        result
    }

    // Returns a copy of self without any liquidity provision interaction.
    pub fn without_onchain_liquidity(&self) -> Self {
        SettlementEncoder {
            tokens: self.tokens.clone(),
            clearing_prices: self.clearing_prices.clone(),
            trades: self.trades.clone(),
            execution_plan: self
                .execution_plan
                .iter()
                // Instead of simply dropping the executions we mark all the interactions as
                // internalizable.
                .map(|(execution, _)| (execution.clone(), true))
                .collect(),
            pre_interactions: self.pre_interactions.clone(),
            unwraps: self.unwraps.clone(),
        }
    }

    pub fn clearing_prices(&self) -> &HashMap<H160, U256> {
        &self.clearing_prices
    }

    pub fn all_trades(&self) -> impl Iterator<Item = PricedTrade> + '_ {
        self.trades
            .iter()
            .map(move |trade| self.compute_trade_token_prices(trade))
    }

    pub fn user_trades(&self) -> impl Iterator<Item = PricedTrade> + '_ {
        self.trades
            .iter()
            .filter(|trade| trade.data.order.is_user_order())
            .map(move |trade| self.compute_trade_token_prices(trade))
    }

    fn compute_trade_token_prices<'a>(&'a self, trade: &'a EncoderTrade) -> PricedTrade<'a> {
        let (sell_token_price, buy_token_price) = match trade.tokens {
            TokenReference::Indexed {
                sell_token_index,
                buy_token_index,
            } => (
                self.clearing_prices[&self.tokens[sell_token_index]],
                self.clearing_prices[&self.tokens[buy_token_index]],
            ),
            TokenReference::CustomPrice {
                sell_token_price,
                buy_token_price,
            } => (sell_token_price, buy_token_price),
        };

        PricedTrade {
            data: &trade.data,
            sell_token_price,
            buy_token_price,
        }
    }

    pub fn has_interactions(&self) -> bool {
        self.execution_plan
            .iter()
            .any(|(_, internalizable)| !internalizable)
    }

    /// Adds an order trade using the uniform clearing prices for sell and buy
    /// token. Fails if any used token doesn't have a price or if executed
    /// amount is impossible.
    fn add_market_trade(
        &mut self,
        order: Order,
        executed_amount: U256,
        scaled_unsubsidized_fee: U256,
    ) -> Result<TradeExecution> {
        verify_executed_amount(&order, executed_amount)?;
        let sell_price = self
            .clearing_prices
            .get(&order.data.sell_token)
            .context("settlement missing sell token")?;
        let sell_token_index = self
            .token_index(order.data.sell_token)
            .expect("missing sell token with price");

        let buy_price = self
            .clearing_prices
            .get(&order.data.buy_token)
            .context("settlement missing buy token")?;
        let buy_token_index = self
            .token_index(order.data.buy_token)
            .expect("missing buy token with price");

        let trade = EncoderTrade {
            data: Trade {
                order: order.clone(),
                executed_amount,
                scaled_unsubsidized_fee,
            },
            tokens: TokenReference::Indexed {
                sell_token_index,
                buy_token_index,
            },
        };
        let execution = trade
            .data
            .executed_amounts(*sell_price, *buy_price)
            .context("impossible trade execution")?;

        self.trades.push(trade);
        Ok(execution)
    }

    /// Adds the passed trade to the execution plan. Handles specifics of
    /// market, limit and liquidity orders internally.
    pub fn add_trade(
        &mut self,
        order: Order,
        executed_amount: U256,
        scaled_unsubsidized_fee: U256,
    ) -> Result<TradeExecution> {
        let interactions = order.interactions.clone();
        let execution = match &order.metadata.class {
            OrderClass::Market => {
                self.add_market_trade(order, executed_amount, scaled_unsubsidized_fee)?
            }
            OrderClass::Liquidity => {
                let (sell_price, buy_price) = (order.data.buy_amount, order.data.sell_amount);
                self.add_custom_price_trade(
                    order,
                    executed_amount,
                    scaled_unsubsidized_fee,
                    sell_price,
                    buy_price,
                )?
            }
            OrderClass::Limit(limit) => {
                // Solvers calculate with slightly adjusted amounts compared to this order but
                // because limit orders are fill-or-kill we can simply use the
                // total original `sell_amount`.
                let executed_amount = match order.data.kind {
                    OrderKind::Sell => order.data.sell_amount,
                    OrderKind::Buy => order.data.buy_amount,
                };
                let (sell_price, buy_price) = self.custom_price_for_limit_order(&order, limit)?;

                self.add_custom_price_trade(
                    order,
                    executed_amount,
                    scaled_unsubsidized_fee,
                    sell_price,
                    buy_price,
                )?
            }
        };
        self.pre_interactions.extend(interactions.pre.into_iter());
        Ok(execution)
    }

    /// Uses the uniform clearing prices to compute the individual buy token
    /// price to satisfy the original limit order which was adjusted to
    /// account for the `surplus_fee` (see
    /// `compute_synthetic_order_amounts_if_limit_order()`).
    /// Returns an error if the UCP doesn't contain the traded tokens or if
    /// under- or overflows happen during the computation.
    fn custom_price_for_limit_order(
        &self,
        order: &Order,
        limit: &LimitOrderClass,
    ) -> Result<(U256, U256)> {
        anyhow::ensure!(
            order.metadata.class.is_limit(),
            "this function should only be called for limit orders"
        );
        // The order passed into this function is the original order signed by the user.
        // But the solver actually computed a solution for an order with `sell_amount -=
        // surplus_fee`. To account for the `surplus_fee` we first have to
        // compute the expected `sell_amount` and `buy_amount` adjusted for the
        // order kind and `surplus_fee`. Afterwards we compute a slightly worse
        // `buy_price` that allows us to buy the expected number of `buy_token`s
        // while pocketing the `surplus_fee` from the `sell_token`s.
        let uniform_buy_price = *self
            .clearing_prices
            .get(&order.data.buy_token)
            .context("buy token price is missing")?;
        let uniform_sell_price = *self
            .clearing_prices
            .get(&order.data.sell_token)
            .context("sell token price is missing")?;

        // Solvable limit orders always have a surplus fee. It would be nice if this was
        // enforced in the API.
        let surplus_fee = limit.surplus_fee.unwrap();
        let (sell_amount, buy_amount) = match order.data.kind {
            // This means sell as much `sell_token` as needed to buy exactly the expected
            // `buy_amount`. Therefore we need to solve for `sell_amount`.
            OrderKind::Buy => {
                let sell_amount = order
                    .data
                    .buy_amount
                    .checked_mul(uniform_buy_price)
                    .context("sell_amount computation failed")?
                    .checked_div(uniform_sell_price)
                    .context("sell_amount computation failed")?;
                // We have to sell slightly more `sell_token` to capture the `surplus_fee`
                let sell_amount_adjusted_for_fees = sell_amount
                    .checked_add(surplus_fee)
                    .context("sell_amount computation failed")?;
                (sell_amount_adjusted_for_fees, order.data.buy_amount)
            }
            // This means sell ALL the `sell_token` and get as much `buy_token` as possible.
            // Therefore we need to solve for `buy_amount`.
            OrderKind::Sell => {
                // Solver actually used this `sell_amount` to compute prices.
                let sell_amount = order
                    .data
                    .sell_amount
                    .checked_sub(surplus_fee)
                    .context("buy_amount computation failed")?;
                let buy_amount = sell_amount
                    .checked_mul(uniform_sell_price)
                    .context("buy_amount computation failed")?
                    .checked_div(uniform_buy_price)
                    .context("buy_amount computation failed")?;
                (order.data.sell_amount, buy_amount)
            }
        };

        let adjusted_sell_price = buy_amount;
        let adjusted_buy_price = sell_amount;
        Ok((adjusted_sell_price, adjusted_buy_price))
    }

    fn add_custom_price_trade(
        &mut self,
        order: Order,
        executed_amount: U256,
        scaled_unsubsidized_fee: U256,
        sell_price: U256,
        buy_price: U256,
    ) -> Result<TradeExecution> {
        verify_executed_amount(&order, executed_amount)?;
        let trade = EncoderTrade {
            data: Trade {
                order,
                executed_amount,
                scaled_unsubsidized_fee,
            },
            tokens: TokenReference::CustomPrice {
                sell_token_price: sell_price,
                buy_token_price: buy_price,
            },
        };
        let execution = trade
            .data
            .executed_amounts(sell_price, buy_price)
            .context("impossible trade execution")?;

        self.trades.push(trade);
        Ok(execution)
    }

    pub fn append_to_execution_plan(&mut self, interaction: impl Interaction + 'static) {
        self.append_to_execution_plan_internalizable(interaction, false)
    }

    pub fn append_to_execution_plan_internalizable(
        &mut self,
        interaction: impl Interaction + 'static,
        internalizable: bool,
    ) {
        self.execution_plan
            .push((Arc::new(interaction), internalizable));
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
            self.trades[i].tokens = match self.trades[i].tokens {
                TokenReference::Indexed { .. } => TokenReference::Indexed {
                    sell_token_index: self
                        .token_index(self.trades[i].data.order.data.sell_token)
                        .expect("missing sell token for existing trade"),
                    buy_token_index: self
                        .token_index(self.trades[i].data.order.data.buy_token)
                        .expect("missing buy token for existing trade"),
                },
                original @ TokenReference::CustomPrice { .. } => original,
            };
        }
    }

    fn token_index(&self, token: H160) -> Option<usize> {
        self.tokens.binary_search(&token).ok()
    }

    /// Returns the total surplus denominated in the native asset for this
    /// solution.
    pub fn total_surplus(&self, external_prices: &ExternalPrices) -> Option<BigRational> {
        self.user_trades().fold(Some(num::zero()), |acc, trade| {
            Some(acc? + trade.surplus_in_native_token(external_prices)?)
        })
    }

    fn drop_unnecessary_tokens_and_prices(&mut self) {
        let traded_tokens: HashSet<_> = self
            .trades
            .iter()
            .flat_map(|trade| {
                // For user order trades, always keep uniform clearing prices
                // for all tokens (even if we could technically skip limit orders).
                if trade.data.order.is_user_order() {
                    Either::Left(
                        [
                            trade.data.order.data.sell_token,
                            trade.data.order.data.buy_token,
                        ]
                        .into_iter(),
                    )
                } else {
                    Either::Right(iter::once(trade.data.order.data.sell_token))
                }
            })
            .collect();

        self.tokens.retain(|token| traded_tokens.contains(token));
        self.clearing_prices
            .retain(|token, _| traded_tokens.contains(token));

        self.sort_tokens_and_update_indices();
    }

    pub fn contains_internalized_interactions(&self) -> bool {
        self.execution_plan
            .iter()
            .any(|(_, internalizable)| *internalizable)
    }

    pub fn finish(
        mut self,
        internalization_strategy: InternalizationStrategy,
    ) -> EncodedSettlement {
        self.drop_unnecessary_tokens_and_prices();

        let uniform_clearing_price_vec_length = self.tokens.len();
        let mut tokens = self.tokens.clone();
        let mut clearing_prices: Vec<U256> = self
            .tokens
            .iter()
            .map(|token| {
                *self
                    .clearing_prices
                    .get(token)
                    .expect("missing clearing price for token")
            })
            .collect();

        {
            // add tokens/prices for custom price orders, since they are not contained in
            // the UCP vector
            let (mut custom_price_order_tokens, mut custom_price_order_prices): (
                Vec<H160>,
                Vec<U256>,
            ) = self
                .trades
                .iter()
                .filter_map(|trade| match trade.tokens {
                    TokenReference::CustomPrice {
                        sell_token_price,
                        buy_token_price,
                    } => Some(vec![
                        (trade.data.order.data.sell_token, sell_token_price),
                        (trade.data.order.data.buy_token, buy_token_price),
                    ]),
                    _ => None,
                })
                .flatten()
                .unzip();

            tokens.append(&mut custom_price_order_tokens);
            clearing_prices.append(&mut custom_price_order_prices);
        }

        let (_, trades) = self.trades.into_iter().fold(
            (uniform_clearing_price_vec_length, Vec::new()),
            |(custom_price_index, mut trades), trade| {
                let (sell_token_index, buy_token_index, custom_price_index) = match trade.tokens {
                    TokenReference::Indexed {
                        sell_token_index,
                        buy_token_index,
                    } => (sell_token_index, buy_token_index, custom_price_index),
                    TokenReference::CustomPrice { .. } => (
                        custom_price_index,
                        custom_price_index + 1,
                        custom_price_index + 2,
                    ),
                };

                trades.push(trade.data.encode(sell_token_index, buy_token_index));
                (custom_price_index, trades)
            },
        );

        EncodedSettlement {
            tokens,
            clearing_prices,
            trades,
            interactions: [
                // In the following it is assumed that all different interactions
                // are only required once to be executed.
                self.pre_interactions
                    .into_iter()
                    .unique()
                    .flat_map(|interaction| interaction.encode())
                    .collect(),
                iter::empty()
                    .chain(
                        self.execution_plan
                            .iter()
                            .filter_map(|(interaction, internalizable)| {
                                if *internalizable
                                    && matches!(
                                        internalization_strategy,
                                        InternalizationStrategy::SkipInternalizableInteraction
                                    )
                                {
                                    None
                                } else {
                                    Some(interaction)
                                }
                            })
                            .flat_map(|interaction| interaction.encode()),
                    )
                    .chain(self.unwraps.iter().flat_map(|unwrap| unwrap.encode()))
                    .collect(),
                Vec::new(),
            ],
        }
    }

    // Merge other into self so that the result contains both settlements.
    // Fails if the settlements cannot be merged for example because the same limit
    // order is used in both or more than one token has a different clearing
    // prices (a single token difference is scaled)
    pub fn merge(mut self, mut other: Self) -> Result<Self> {
        let scaling_factor = self.price_scaling_factor(&other);
        // Make sure we always scale prices up to avoid precision issues
        if scaling_factor < BigRational::one() {
            return other.merge(self);
        }

        for (key, value) in &other.clearing_prices {
            let scaled_price = big_rational_to_u256(&(value.to_big_rational() * &scaling_factor))
                .context("Invalid price scaling factor")?;
            match self.clearing_prices.entry(*key) {
                Entry::Occupied(entry) => ensure!(
                    *entry.get() == scaled_price,
                    "different price after scaling"
                ),
                Entry::Vacant(entry) => {
                    entry.insert(scaled_price);
                    self.tokens.push(*key);
                }
            }
        }

        for that in other.trades.iter() {
            ensure!(
                self.trades
                    .iter()
                    .all(|this| this.data.order.metadata.uid != that.data.order.metadata.uid),
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
    /// This can be used in case the settlement contracts ETH buffer is big
    /// enough.
    pub fn drop_unwrap(&mut self, token: H160) {
        self.unwraps.retain(|unwrap| unwrap.weth.address() != token);
    }

    /// Calculates how much of a given token this settlement will unwrap during
    /// the execution.
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

impl PricedTrade<'_> {
    pub fn surplus_in_native_token(&self, external_prices: &ExternalPrices) -> Option<BigRational> {
        trade_surplus_in_native_token_with_prices(
            &self.data.order,
            self.data.executed_amount,
            external_prices,
            self.sell_token_price,
            self.buy_token_price,
        )
    }

    pub fn executed_amounts(&self) -> Option<TradeExecution> {
        self.data
            .executed_amounts(self.sell_token_price, self.buy_token_price)
    }
}

pub fn verify_executed_amount(order: &Order, executed: U256) -> Result<()> {
    let remaining = shared::remaining_amounts::Remaining::from_order(order)?;
    let valid_executed_amount = match (order.data.partially_fillable, order.data.kind) {
        (true, OrderKind::Sell) => executed <= remaining.remaining(order.data.sell_amount)?,
        (true, OrderKind::Buy) => executed <= remaining.remaining(order.data.buy_amount)?,
        (false, OrderKind::Sell) => executed == remaining.remaining(order.data.sell_amount)?,
        (false, OrderKind::Buy) => executed == remaining.remaining(order.data.buy_amount)?,
    };
    ensure!(valid_executed_amount, "invalid executed amount");
    Ok(())
}

#[cfg(test)]
pub mod tests {
    use {
        super::*,
        crate::settlement::NoopInteraction,
        contracts::WETH9,
        ethcontract::Bytes,
        maplit::hashmap,
        model::order::{OrderBuilder, OrderData},
        shared::{
            dummy_contract,
            interaction::{EncodedInteraction, Interaction},
        },
    };

    #[test]
    pub fn encode_trades_finds_token_index() {
        let token0 = H160::from_low_u64_be(0);
        let token1 = H160::from_low_u64_be(1);
        let order0 = Order {
            data: OrderData {
                sell_token: token0,
                sell_amount: 1.into(),
                buy_token: token1,
                buy_amount: 1.into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let order1 = Order {
            data: OrderData {
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

        assert!(settlement.add_trade(order0, 1.into(), 1.into()).is_ok());
        assert!(settlement.add_trade(order1, 1.into(), 0.into()).is_ok());
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
            encoder
                .finish(InternalizationStrategy::SkipInternalizableInteraction)
                .interactions[1],
            UnwrapWethInteraction {
                weth,
                amount: 3.into(),
            }
            .encode(),
        );
    }

    #[test]
    fn settlement_reflects_different_price_for_normal_and_liquidity_order() {
        let mut settlement = SettlementEncoder::new(maplit::hashmap! {
            token(0) => 3.into(),
            token(1) => 10.into(),
        });

        let order01 = OrderBuilder::default()
            .with_sell_token(token(0))
            .with_sell_amount(30.into())
            .with_buy_token(token(1))
            .with_buy_amount(10.into())
            .build();

        let order10 = OrderBuilder::default()
            .with_sell_token(token(1))
            .with_sell_amount(10.into())
            .with_buy_token(token(0))
            .with_buy_amount(20.into())
            .with_class(OrderClass::Liquidity)
            .build();

        assert!(settlement.add_trade(order01, 10.into(), 0.into()).is_ok());
        assert!(settlement.add_trade(order10, 20.into(), 0.into()).is_ok());
        let finished_settlement =
            settlement.finish(InternalizationStrategy::SkipInternalizableInteraction);
        assert_eq!(
            finished_settlement.tokens,
            vec![token(0), token(1), token(1), token(0)]
        );
        assert_eq!(
            finished_settlement.clearing_prices,
            vec![3.into(), 10.into(), 20.into(), 10.into()]
        );
        assert_eq!(
            finished_settlement.trades[1].1, // <-- is the buy token index of liquidity order
            3.into()
        );
        assert_eq!(
            finished_settlement.trades[0].1, // <-- is the buy token index of normal order
            1.into()
        );
    }

    #[test]
    fn settlement_inserts_sell_price_for_new_liquidity_order_if_price_did_not_exist() {
        let mut settlement = SettlementEncoder::new(maplit::hashmap! {
            token(1) => 9.into(),
        });
        let order01 = OrderBuilder::default()
            .with_sell_token(token(0))
            .with_sell_amount(30.into())
            .with_buy_token(token(1))
            .with_buy_amount(10.into())
            .with_class(OrderClass::Liquidity)
            .build();
        assert!(settlement
            .add_trade(order01.clone(), 10.into(), 0.into())
            .is_ok());
        // ensures that the output of add_liquidity_order is not changed after adding
        // liquidity order
        assert_eq!(settlement.tokens, vec![token(1)]);
        let finished_settlement =
            settlement.finish(InternalizationStrategy::SkipInternalizableInteraction);
        // the initial price from:SettlementEncoder::new(maplit::hashmap! {
        //     token(1) => 9.into(),
        // });
        // gets dropped and replaced by the liquidity price
        assert_eq!(finished_settlement.tokens, vec![token(0), token(1)]);
        assert_eq!(
            finished_settlement.clearing_prices,
            vec![order01.data.buy_amount, order01.data.sell_amount]
        );
        assert_eq!(
            finished_settlement.trades[0].0, // <-- is the sell token index of liquidity order
            0.into()
        );
        assert_eq!(
            finished_settlement.trades[0].1, // <-- is the buy token index of liquidity order
            1.into()
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
            encoder
                .finish(InternalizationStrategy::SkipInternalizableInteraction)
                .interactions[1],
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
                    data: OrderData {
                        sell_token: token_a,
                        sell_amount: 6.into(),
                        buy_token: token_b,
                        buy_amount: 3.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                3.into(),
                0.into(),
            )
            .unwrap();

        assert_eq!(encoder.tokens, [token_a, token_b]);
        assert_eq!(
            encoder.trades[0].tokens,
            TokenReference::Indexed {
                sell_token_index: 0,
                buy_token_index: 1,
            }
        );

        let token_c = H160([0xee; 20]);
        encoder.add_token_equivalency(token_a, token_c).unwrap();

        assert_eq!(encoder.tokens, [token_a, token_c, token_b]);
        assert_eq!(
            encoder.clearing_prices[&token_a],
            encoder.clearing_prices[&token_c],
        );
        assert_eq!(
            encoder.trades[0].tokens,
            TokenReference::Indexed {
                sell_token_index: 0,
                buy_token_index: 2,
            }
        );
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
    fn merge_ok_with_liquidity_orders() {
        let weth = dummy_contract!(WETH9, H160::zero());
        let prices = hashmap! { token(1) => 1.into(), token(3) => 3.into() };
        let mut encoder0 = SettlementEncoder::new(prices);
        let mut order13 = OrderBuilder::default()
            .with_sell_token(token(1))
            .with_sell_amount(33.into())
            .with_buy_token(token(3))
            .with_buy_amount(11.into())
            .build();
        let mut order12 = OrderBuilder::default()
            .with_sell_token(token(1))
            .with_sell_amount(23.into())
            .with_buy_token(token(2))
            .with_buy_amount(11.into())
            .with_class(OrderClass::Liquidity)
            .build();
        order13.metadata.uid.0[0] = 0;
        order12.metadata.uid.0[0] = 2;
        encoder0
            .add_trade(order13.clone(), 11.into(), 0.into())
            .unwrap();
        encoder0
            .add_trade(order12.clone(), 11.into(), 0.into())
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
        let mut order23 = OrderBuilder::default()
            .with_sell_token(token(2))
            .with_sell_amount(19.into())
            .with_buy_token(token(3))
            .with_buy_amount(11.into())
            .with_class(OrderClass::Liquidity)
            .build();
        order24.metadata.uid.0[0] = 1;
        order23.metadata.uid.0[0] = 4;
        encoder1
            .add_trade(order24.clone(), 22.into(), 0.into())
            .unwrap();
        encoder1
            .add_trade(order23.clone(), 11.into(), 0.into())
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
        assert_eq!(
            merged.trades,
            [
                EncoderTrade {
                    data: Trade {
                        order: order13,
                        executed_amount: 11.into(),
                        scaled_unsubsidized_fee: 0.into()
                    },
                    tokens: TokenReference::Indexed {
                        sell_token_index: 0,
                        buy_token_index: 2,
                    },
                },
                EncoderTrade {
                    data: Trade {
                        order: order12,
                        executed_amount: 11.into(),
                        scaled_unsubsidized_fee: 0.into()
                    },
                    tokens: TokenReference::CustomPrice {
                        sell_token_price: 11.into(),
                        buy_token_price: 23.into(),
                    },
                },
                EncoderTrade {
                    data: Trade {
                        order: order24,
                        executed_amount: 22.into(),
                        scaled_unsubsidized_fee: 0.into()
                    },
                    tokens: TokenReference::Indexed {
                        sell_token_index: 1,
                        buy_token_index: 3,
                    },
                },
                EncoderTrade {
                    data: Trade {
                        order: order23,
                        executed_amount: 11.into(),
                        scaled_unsubsidized_fee: 0.into()
                    },
                    tokens: TokenReference::CustomPrice {
                        sell_token_price: 11.into(),
                        buy_token_price: 19.into(),
                    },
                },
            ],
        );

        assert_eq!(merged.trades.len(), 4);
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
        let mut encoder1 = SettlementEncoder::new(prices);
        let mut order = Order::default();
        order.data.buy_token = token(1);
        order.data.sell_token = token(3);

        encoder1.trades = vec![EncoderTrade {
            data: Trade {
                order: order.clone(),
                ..Default::default()
            },
            tokens: TokenReference::CustomPrice {
                sell_token_price: 3.into(),
                buy_token_price: 5.into(),
            },
        }];
        let merged = encoder0.merge(encoder1).unwrap();
        let prices = hashmap! {
            token(1) => 2.into(),
            token(2) => 2.into(),
            token(3) => 6.into(),
        };
        assert_eq!(merged.clearing_prices, prices);
        assert_eq!(
            merged.trades,
            vec![EncoderTrade {
                data: Trade {
                    order,
                    ..Default::default()
                },
                tokens: TokenReference::CustomPrice {
                    // no price was changed, because custom price orders have their prices outside
                    // of UCP vector and their value is not correlated with UCP whatsoever.
                    sell_token_price: 3.into(),
                    buy_token_price: 5.into(),
                },
            }],
        );
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
            .add_trade(order13.clone(), 11.into(), 0.into())
            .unwrap();

        let mut encoder1 = SettlementEncoder::new(prices);
        encoder1.add_trade(order13, 11.into(), 0.into()).unwrap();

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
        encoder.add_trade(order_1_3, 11.into(), 0.into()).unwrap();

        let weth = dummy_contract!(WETH9, token(2));
        encoder.add_unwrap(UnwrapWethInteraction {
            weth,
            amount: 12.into(),
        });

        let encoded = encoder.finish(InternalizationStrategy::SkipInternalizableInteraction);

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

    #[derive(Debug)]
    pub struct TestInteraction;
    impl Interaction for TestInteraction {
        fn encode(&self) -> Vec<EncodedInteraction> {
            vec![(H160::zero(), U256::zero(), Bytes::default())]
        }
    }

    #[test]
    fn optionally_encodes_internalizable_transactions() {
        let prices = hashmap! {token(1) => 7.into() };

        let mut encoder = SettlementEncoder::new(prices);
        encoder.append_to_execution_plan_internalizable(TestInteraction, true);
        encoder.append_to_execution_plan_internalizable(TestInteraction, false);

        let encoded = encoder
            .clone()
            .finish(InternalizationStrategy::SkipInternalizableInteraction);
        assert_eq!(encoded.interactions[1].len(), 1);

        let encoded = encoder.finish(InternalizationStrategy::EncodeAllInteractions);
        assert_eq!(encoded.interactions[1].len(), 2);
    }

    #[test]
    fn computes_custom_price_for_sell_limit_order_correctly() {
        let weth = token(1);
        let usdc = token(2);
        let prices = hashmap! {
            // assumption 1 WETH == 1_000 USDC (all prices multiplied by 10^18)
            weth => U256::exp10(18),
            usdc => U256::exp10(27) // 1 ETH buys 1_000 * 10^6 units of USDC
        };

        let mut encoder = SettlementEncoder::new(prices);
        // sell 1.01 WETH for 1_000 USDC with a fee of 0.01 WETH (or 10 USDC)
        let order = OrderBuilder::default()
            .with_class(OrderClass::Limit(Default::default()))
            .with_sell_token(weth)
            .with_sell_amount(1_010_000_000_000_000_000u128.into()) // 1.01 WETH
            .with_buy_token(usdc)
            .with_buy_amount(U256::exp10(9)) // 1_000 USDC
            .with_surplus_fee(U256::exp10(16)) // 0.01 WETH
            .with_fee_amount(0.into())
            .with_kind(OrderKind::Sell)
            .build();

        let execution = encoder
            .add_trade(order.clone(), U256::exp10(18), U256::exp10(16))
            .unwrap();
        assert_eq!(
            TradeExecution {
                sell_token: weth,
                buy_token: usdc,
                sell_amount: 1_010_000_000_000_000_000u128.into(), // 1.01 WETH
                buy_amount: U256::exp10(9),                        // 1_000 USDC
                fee_amount: 0.into(),
            },
            execution
        );
        assert_eq!(
            EncoderTrade {
                data: Trade {
                    order,
                    executed_amount: 1_010_000_000_000_000_000u128.into(), // 1.01 WETH
                    scaled_unsubsidized_fee: U256::exp10(16)               // 0.01 WETH (10 USDC)
                },
                tokens: TokenReference::CustomPrice {
                    sell_token_price: U256::exp10(9),
                    // Instead of the (solver) anticipated 1 WETH required to buy 1_000 USDC we had
                    // to sell 1.01 WETH (to pocket the fee). This caused the
                    // USDC price to increase by 1%.
                    buy_token_price: 1_010_000_000_000_000_000u128.into()
                },
            },
            encoder.trades[0]
        );
    }

    #[test]
    fn computes_custom_price_for_buy_limit_order_correctly() {
        let weth = token(1);
        let usdc = token(2);
        // assuming 1 WETH == 1_000 USDC
        let prices = hashmap! {
            weth => U256::exp10(18),
            usdc => U256::exp10(27) // 1 ETH buys 1_000 * 10^6 units of USDC
        };

        let mut encoder = SettlementEncoder::new(prices);
        // buy 1 WETH for 1_010 USDC with a fee of 10 USDC
        let order = OrderBuilder::default()
            .with_class(OrderClass::Limit(Default::default()))
            .with_buy_token(weth)
            .with_buy_amount(U256::exp10(18)) // 1 WETH
            .with_sell_token(usdc)
            .with_sell_amount(1_010_000_000u128.into()) // 1_010 USDC
            .with_surplus_fee(U256::exp10(7)) // 10 USDC
            .with_fee_amount(0.into())
            .with_kind(OrderKind::Buy)
            .build();

        let execution = encoder
            .add_trade(order.clone(), U256::exp10(18), U256::exp10(7))
            .unwrap();
        assert_eq!(
            TradeExecution {
                sell_token: usdc,
                buy_token: weth,
                // With the original price selling 1_000 USDC would have been enough.
                // With the adjusted price we actually have to sell all 1_010 USDC.
                sell_amount: 1_010_000_000u128.into(), // 1_010 USDC
                buy_amount: U256::exp10(18),           // 1 WETH
                fee_amount: 0.into(),
            },
            execution
        );
        assert_eq!(
            EncoderTrade {
                data: Trade {
                    order,
                    executed_amount: U256::exp10(18), // 1 WETH
                    scaled_unsubsidized_fee: U256::exp10(7)  // 10 USDC
                },
                tokens: TokenReference::CustomPrice {
                    sell_token_price: U256::exp10(18),
                    // Instead of the (solver) anticipated 1_000 USDC required to buy 1 WETH we had
                    // to sell 1_010 USDC (to pocket the fee). This caused the
                    // WETH price to increase by 1%.
                    buy_token_price: 1_010_000_000u128.into()
                }
            },
            encoder.trades[0]
        );
    }
}
