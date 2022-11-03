use super::{CustomPriceTrade, ExternalPrices, OrderTrade, Trade, TradeExecution};
use crate::{
    encoding::{EncodedSettlement, EncodedTrade},
    interactions::UnwrapWethInteraction,
    settlement::trade_surplus_in_native_token,
};
use anyhow::{bail, ensure, Context as _, Result};
use itertools::Itertools;
use model::{
    interaction::InteractionData,
    order::{Order, OrderClass, OrderKind},
};
use num::{BigRational, One};
use number_conversions::big_rational_to_u256;
use primitive_types::{H160, U256};
use shared::{conversions::U256Ext, interaction::Interaction};
use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    iter,
    sync::Arc,
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
#[derive(Debug, Clone)]
pub struct SettlementEncoder {
    // Make sure to update the `merge` method when adding new fields.

    // Invariant: tokens is all keys in clearing_prices sorted.
    tokens: Vec<H160>,
    clearing_prices: HashMap<H160, U256>,
    // Order trades are trades of usual user orders. They will be settled using the
    // uniform clearing prices. Hence, every trade's buy and sell token has an entry
    // in clearing_prices
    order_trades: Vec<OrderTrade>,
    custom_price_trades: Vec<CustomPriceTrade>,
    // This is an Arc so that this struct is Clone. Cannot require `Interaction: Clone` because it
    // would make the trait not be object safe which prevents using it through `dyn`.
    // TODO: Can we fix this in a better way?
    execution_plan: Vec<MaybeInternalizableInteraction>,
    pre_interactions: Vec<InteractionData>,
    unwraps: Vec<UnwrapWethInteraction>,
}

impl Default for SettlementEncoder {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

/// Whether or not internalizable interactions should be encoded as calldata
pub enum InternalizationStrategy {
    EncodeAllInteractions,
    SkipInternalizableInteraction,
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
            order_trades: Vec::new(),
            custom_price_trades: Vec::new(),
            execution_plan: Vec::new(),
            pre_interactions: Vec::new(),
            unwraps: Vec::new(),
        }
    }

    #[cfg(test)]
    pub fn with_trades(
        clearing_prices: HashMap<H160, U256>,
        trades: Vec<OrderTrade>,
        custom_price_trades: Vec<CustomPriceTrade>,
    ) -> Self {
        let mut result = Self::new(clearing_prices);
        result.order_trades = trades;
        result.custom_price_trades = custom_price_trades;
        result
    }

    // Returns a copy of self without any liquidity provision interaction.
    pub fn without_onchain_liquidity(&self) -> Self {
        SettlementEncoder {
            tokens: self.tokens.clone(),
            clearing_prices: self.clearing_prices.clone(),
            order_trades: self.order_trades.clone(),
            custom_price_trades: self.custom_price_trades.clone(),
            execution_plan: Vec::new(),
            pre_interactions: self.pre_interactions.clone(),
            unwraps: self.unwraps.clone(),
        }
    }

    pub fn clearing_prices(&self) -> &HashMap<H160, U256> {
        &self.clearing_prices
    }

    pub fn order_trades(&self) -> &[OrderTrade] {
        &self.order_trades
    }

    pub fn custom_price_trades(&self) -> &[CustomPriceTrade] {
        &self.custom_price_trades
    }

    pub fn has_interactions(&self) -> bool {
        self.execution_plan
            .iter()
            .any(|(_, internalizable)| !internalizable)
    }

    /// Adds an order trade using the uniform clearing prices for sell and buy token.
    /// Fails if any used token doesn't have a price or if executed amount is impossible.
    fn add_market_trade(
        &mut self,
        order: Order,
        executed_amount: U256,
        scaled_unsubsidized_fee: U256,
    ) -> Result<TradeExecution> {
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

        let order_trade = OrderTrade {
            trade: Trade {
                order: order.clone(),
                sell_token_index,
                executed_amount,
                scaled_unsubsidized_fee,
            },
            buy_token_index,
        };
        let execution = order_trade
            .trade
            .executed_amounts(*sell_price, *buy_price)
            .context("impossible trade execution")?;

        self.order_trades.push(order_trade);
        Ok(execution)
    }

    /// Adds the passed trade to the execution plan. Handles specifics of ordinary, limit and
    /// liquidity orders internally.
    pub fn add_trade(
        &mut self,
        order: Order,
        executed_amount: U256,
        scaled_unsubsidized_fee: U256,
    ) -> Result<TradeExecution> {
        verify_executed_amount(&order, executed_amount)?;
        let interactions = order.interactions.clone();
        let execution = match order.metadata.class {
            OrderClass::Ordinary => {
                self.add_market_trade(order, executed_amount, scaled_unsubsidized_fee)?
            }
            OrderClass::Liquidity => {
                let buy_price = self.compute_limit_buy_price(&order)?;
                self.add_custom_price_trade(
                    order,
                    executed_amount,
                    scaled_unsubsidized_fee,
                    buy_price,
                )?
            }
            OrderClass::Limit => {
                // Solvers calculate with slightly adjusted amounts compared to this order but because
                // limit orders are fill-or-kill we can simply use the total original `sell_amount`.
                let executed_amount = order.data.sell_amount;
                let buy_price = self.custom_price_for_limit_order(&order)?;

                self.add_custom_price_trade(
                    order,
                    executed_amount,
                    scaled_unsubsidized_fee,
                    buy_price,
                )?
            }
        };
        self.pre_interactions.extend(interactions.pre.into_iter());
        Ok(execution)
    }

    /// Uses the uniform clearing prices to compute the individual buy token price to satisfy the
    /// original limit order which was adjusted to account for the `surplus_fee` (see
    /// `compute_synthetic_order_amounts_if_limit_order()`).
    /// Returns an error if the UCP doesn't contain the `sell_token` or if under- or overflows
    /// happen during the computation.
    fn custom_price_for_limit_order(&self, order: &Order) -> Result<U256> {
        // The order passed into this function is the original order signed by the user.
        // But the solver actually computated a solution for an order with `sell_amount -= surplus_fee.
        // That means this is the equation the solver tried to satisfy:
        // uniform_sell_price * (sell_amount - surplus_fee) = uniform_buy_price * buy_amount
        //
        // But actually this is the equation the solution has to satisfy:
        // uniform_sell_price * sell_amount = custom_buy_price * buy_amount
        // That means we can solve for the custom_buy_price like this:
        // custom_buy_price = (uniform_sell_price * sell_amount) / buy_amount

        let uniform_sell_price = self
            .clearing_prices()
            .get(&order.data.sell_token)
            .context("missing sell token price")?;

        order
            .data
            .sell_amount
            .checked_mul(*uniform_sell_price)
            .context("buy_price calculation failed")?
            .checked_div(order.data.buy_amount)
            .context("buy price calculation failed")
    }

    /// Uses either the existing UCP for the sell token or the sell price within the order to
    /// compute the price of the buy token right at the limit price of the passed order.
    /// This is needed to settle liquidity orders in a way that gives them no surplus.
    fn compute_limit_buy_price(&self, order: &Order) -> Result<U256> {
        // Liquidity orders are settled at their limit price. We set:
        // buy_price = sell_price * order.sellAmount / order.buyAmount, where sell_price is given from uniform clearing prices

        // Rounding error checks:
        // Following limit price constraint is checked in the smart contract:
        // order.sellAmount.mul(sellPrice) >= order.buyAmount.mul(buyPrice),

        // This equation will always be true, as the following equivalence is showing
        // order.sellAmount.mul(sellPrice)  >= order.sellAmount.mul(sellPrice) holds, as the left and ride side are equal.
        // Dividing first and then multiplying by order.buyAmount can only make the right side smaller
        // <=> order.sellAmount.mul(sellPrice)  >= order.buyAmount.mul(sell_price * order.sellAmount / order.buyAmount)
        // <=> order.sellAmount.mul(sellPrice)  >= order.buyAmount.mul(buyPrice)
        // <=> equation from smart contract

        self.clearing_prices
            .get(&order.data.sell_token)
            .unwrap_or(&order.data.buy_amount)
            .checked_mul(order.data.sell_amount)
            .context("buy_price calculation failed")?
            .checked_div(order.data.buy_amount)
            .context("buy_price calculation failed")
    }

    fn add_custom_price_trade(
        &mut self,
        order: Order,
        executed_amount: U256,
        scaled_unsubsidized_fee: U256,
        buy_price: U256,
    ) -> Result<TradeExecution> {
        // For the encoding strategy of liquidity and limit orders, the sell prices are taken from
        // the uniform clearing price vector. Therefore, either there needs to be an existing price
        // for the sell token in the uniform clearing prices or we have to create a new price entry beforehand,
        // if the sell token price is not yet available
        if self.token_index(order.data.sell_token).is_none() {
            let sell_token = order.data.sell_token;
            let sell_price = order.data.buy_amount;
            self.tokens.push(sell_token);
            self.clearing_prices.insert(sell_token, sell_price);
            self.sort_tokens_and_update_indices();
        };
        let sell_price = self
            .clearing_prices
            .get(&order.data.sell_token)
            .context("settlement missing sell token")?;
        let sell_token_index = self
            .token_index(order.data.sell_token)
            .context("settlement missing sell token")?;

        let trade = Trade {
            order: order.clone(),
            sell_token_index,
            executed_amount,
            scaled_unsubsidized_fee,
        };
        let custom_price_trade = CustomPriceTrade {
            trade,
            buy_token_offset_index: self.custom_price_trades.len(),
            buy_token_price: buy_price,
        };
        let execution = custom_price_trade
            .trade
            .executed_amounts(*sell_price, buy_price)
            .context("impossible trade execution")?;

        self.custom_price_trades.push(custom_price_trade);
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
        for i in 0..self.order_trades.len() {
            self.order_trades[i].trade.sell_token_index = self
                .token_index(self.order_trades[i].trade.order.data.sell_token)
                .expect("missing sell token for existing trade");

            self.order_trades[i].buy_token_index = self
                .token_index(self.order_trades[i].trade.order.data.buy_token)
                .expect("missing buy token for existing trade");
        }
        for i in 0..self.custom_price_trades.len() {
            self.custom_price_trades[i].trade.sell_token_index = self
                .token_index(self.custom_price_trades[i].trade.order.data.sell_token)
                .expect("missing sell token for existing trade");
        }
    }
    fn modify_token_index_for_liquidity_orders_after_change(&mut self, offset: usize) {
        for custom_price_trade in &mut self.custom_price_trades {
            custom_price_trade.buy_token_offset_index += offset;
        }
    }

    fn token_index(&self, token: H160) -> Option<usize> {
        self.tokens.binary_search(&token).ok()
    }

    /// Returns the total surplus denominated in the native asset for this
    /// solution.
    pub fn total_surplus(&self, external_prices: &ExternalPrices) -> Option<BigRational> {
        self.order_trades
            .iter()
            .fold(Some(num::zero()), |acc, order_trade| {
                let normalized_surplus = trade_surplus_in_native_token(
                    &order_trade.trade.order,
                    order_trade.trade.executed_amount,
                    external_prices,
                    &self.clearing_prices,
                )?;
                Some(acc? + normalized_surplus)
            })
    }

    fn drop_unnecessary_tokens_and_prices(&mut self) {
        let traded_tokens: HashSet<_> = self
            .order_trades
            .iter()
            .flat_map(|order_trade| {
                [
                    order_trade.trade.order.data.buy_token,
                    order_trade.trade.order.data.sell_token,
                ]
            })
            .collect();

        let liquidity_traded_sell_tokens: HashSet<_> = self
            .custom_price_trades
            .iter()
            .map(|liquidity_order_trade| liquidity_order_trade.trade.order.data.sell_token)
            .collect();

        self.tokens.retain(|token| {
            traded_tokens.contains(token) || liquidity_traded_sell_tokens.contains(token)
        });
        self.sort_tokens_and_update_indices();
        self.clearing_prices.retain(|token, _price| {
            traded_tokens.contains(token) || liquidity_traded_sell_tokens.contains(token)
        });
    }

    pub fn finish(
        mut self,
        internalization_strategy: InternalizationStrategy,
    ) -> EncodedSettlement {
        self.drop_unnecessary_tokens_and_prices();

        let (mut liquidity_order_buy_tokens, mut liquidity_order_prices): (Vec<H160>, Vec<U256>) =
            self.custom_price_trades
                .iter()
                .map(|liquidity_order_trade| {
                    (
                        liquidity_order_trade.trade.order.data.buy_token,
                        liquidity_order_trade.buy_token_price,
                    )
                })
                .unzip();
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
        tokens.append(&mut liquidity_order_buy_tokens);
        clearing_prices.append(&mut liquidity_order_prices);
        let mut trades: Vec<EncodedTrade> = self
            .order_trades
            .into_iter()
            .map(|trade| trade.encode())
            .collect();
        let mut liquidity_order_trades: Vec<EncodedTrade> = self
            .custom_price_trades
            .into_iter()
            .map(|trade| trade.encode(uniform_clearing_price_vec_length))
            .collect();
        trades.append(&mut liquidity_order_trades);
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
    // Fails if the settlements cannot be merged for example because the same limit order is used in
    // both or more than one token has a different clearing prices (a single token difference is scaled)
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
        other.custom_price_trades = other
            .custom_price_trades
            .iter()
            .map(|liquidity_order| {
                let buy_token_price = big_rational_to_u256(
                    &(liquidity_order.buy_token_price.to_big_rational() * &scaling_factor),
                )
                .context("Invalid price scaling factor")?;
                Ok(CustomPriceTrade {
                    trade: liquidity_order.trade.clone(),
                    buy_token_offset_index: liquidity_order.buy_token_offset_index,
                    buy_token_price,
                })
            })
            .collect::<Result<Vec<CustomPriceTrade>>>()?;

        for other_order_trade in other.order_trades.iter() {
            ensure!(
                self.order_trades
                    .iter()
                    .all(|self_order_trade| self_order_trade.trade.order.metadata.uid
                        != other_order_trade.trade.order.metadata.uid),
                "duplicate normal trade"
            );
        }

        for other_liquidity_order_trade in other.custom_price_trades.iter() {
            ensure!(
                self.custom_price_trades
                    .iter()
                    .all(|self_liquidity_order_trade| self_liquidity_order_trade
                        .trade
                        .order
                        .metadata
                        .uid
                        != other_liquidity_order_trade.trade.order.metadata.uid),
                "duplicate liquidity trade"
            );
        }

        other.modify_token_index_for_liquidity_orders_after_change(self.custom_price_trades.len());
        self.custom_price_trades
            .append(&mut other.custom_price_trades);
        self.order_trades.append(&mut other.order_trades);
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
    use super::*;
    use crate::{encoding::EncodedInteraction, settlement::NoopInteraction};
    use contracts::WETH9;
    use ethcontract::Bytes;
    use maplit::hashmap;
    use model::order::{OrderBuilder, OrderData};
    use shared::{dummy_contract, interaction::Interaction};

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
            vec![token(0), token(1), token(0)]
        );
        assert_eq!(
            finished_settlement.clearing_prices,
            vec![3.into(), 10.into(), 5.into()]
        );
        assert_eq!(
            finished_settlement.trades[1].1, // <-- is the buy token index of liquidity order
            2.into()
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
        // ensures that the output of add_liquidity_order is sorted
        assert_eq!(settlement.tokens, vec![token(0), token(1)]);
        assert_eq!(settlement.custom_price_trades[0].trade.sell_token_index, 0);
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
        assert_eq!(encoder.order_trades[0].trade.sell_token_index, 0);
        assert_eq!(encoder.order_trades[0].buy_token_index, 1);

        let token_c = H160([0xee; 20]);
        encoder.add_token_equivalency(token_a, token_c).unwrap();

        assert_eq!(encoder.tokens, [token_a, token_c, token_b]);
        assert_eq!(
            encoder.clearing_prices[&token_a],
            encoder.clearing_prices[&token_c],
        );
        assert_eq!(encoder.order_trades[0].trade.sell_token_index, 0);
        assert_eq!(encoder.order_trades[0].buy_token_index, 2);
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
        encoder0.add_trade(order13, 11.into(), 0.into()).unwrap();
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
        encoder1.add_trade(order24, 22.into(), 0.into()).unwrap();
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
            merged.custom_price_trades,
            vec![
                CustomPriceTrade {
                    trade: Trade {
                        order: order12,
                        sell_token_index: 0,
                        executed_amount: 11.into(),
                        scaled_unsubsidized_fee: 0.into()
                    },
                    buy_token_offset_index: 0,
                    buy_token_price: 2.into(),
                },
                CustomPriceTrade {
                    trade: Trade {
                        order: order23,
                        sell_token_index: 1,
                        executed_amount: 11.into(),
                        scaled_unsubsidized_fee: 0.into()
                    },
                    buy_token_offset_index: 1,
                    buy_token_price: 3.into(),
                }
            ]
        );

        assert_eq!(merged.order_trades.len(), 2);
        assert_eq!(merged.custom_price_trades.len(), 2);
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

        encoder1.custom_price_trades = vec![CustomPriceTrade {
            trade: Trade {
                order: order.clone(),
                ..Default::default()
            },
            buy_token_price: 1.into(),
            ..Default::default()
        }];
        let merged = encoder0.merge(encoder1).unwrap();
        let prices = hashmap! {
            token(1) => 2.into(),
            token(2) => 2.into(),
            token(3) => 6.into(),
        };
        assert_eq!(merged.clearing_prices, prices);
        assert_eq!(
            merged.custom_price_trades,
            vec![CustomPriceTrade {
                trade: Trade {
                    order,
                    sell_token_index: 2,
                    ..Default::default()
                },
                buy_token_price: 2.into(),
                ..Default::default()
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
}
