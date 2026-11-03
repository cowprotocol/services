//! Protocol fee policies and the fee math over a fill.
//!
//! Generic over the chain's type vocabulary ([`ChainTypes`]). [`FeePolicy`],
//! [`Quote`] and [`Order`] default their chain parameter to [`Evm`], so their
//! bare names mean the EVM instantiation.

use chain_types::{Amount, ChainTypes, MathError, MathResult, Zero, evm::Evm};

#[cfg(test)]
mod tests;

/// Order side (buy or sell).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    Buy,
    Sell,
}

/// Protocol fee policy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FeePolicy<C: ChainTypes = Evm> {
    /// Fee as a percentage of surplus over limit price.
    Surplus { factor: f64, max_volume_factor: f64 },
    /// Fee as a percentage of price improvement over quote.
    PriceImprovement {
        factor: f64,
        max_volume_factor: f64,
        quote: Quote<C>,
    },
    /// Fee as a percentage of order volume.
    Volume { factor: f64 },
}

/// Quote data for price improvement fee calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quote<C: ChainTypes = Evm> {
    pub sell_amount: C::Amount,
    pub buy_amount: C::Amount,
    pub fee: C::Amount,
    pub solver: C::AccountId,
}

/// An order's limit amounts and the amounts a solution executed for it.
#[derive(Debug, Clone)]
pub struct Order<C: ChainTypes = Evm> {
    /// Unique order identifier.
    pub uid: C::OrderUid,

    /// Sell token address.
    pub sell_token: C::TokenId,

    /// Buy token address.
    pub buy_token: C::TokenId,

    /// Limit amount of sell token (from original order parameters).
    ///
    /// This is the maximum amount the user is willing to sell.
    pub sell_amount: C::Amount,

    /// Limit amount of buy token (from original order parameters).
    ///
    /// This is the minimum amount the user wants to receive.
    pub buy_amount: C::Amount,

    /// Amount of sell token that left the user's wallet (including fees).
    ///
    /// This is the actual executed amount in this solution.
    pub executed_sell: C::Amount,

    /// Amount of buy token the user received (after fees).
    ///
    /// This is the actual amount the user got in this solution.
    pub executed_buy: C::Amount,

    /// Order side (Buy or Sell).
    ///
    /// Determines how surplus is calculated.
    pub side: Side,
}

/// Clearing prices for a trade.
///
/// These can be either uniform (same for all orders) or custom (adjusted for
/// protocol fees on a per-order basis).
#[derive(Debug, Clone, Copy)]
struct ClearingPrices<C: ChainTypes> {
    /// Price of sell token in terms of buy token.
    sell: C::Amount,
    /// Price of buy token in terms of sell token.
    buy: C::Amount,
}

/// Price limits for an order or quote.
#[derive(Debug, Clone, Copy)]
struct PriceLimits<C: ChainTypes> {
    /// Maximum sell amount.
    sell: C::Amount,
    /// Minimum buy amount.
    buy: C::Amount,
}

/// The fee each of `policies` took from the fill `order` describes, in list
/// order and in the surplus token, recovered from the settled legs. The last
/// policy is measured at the settled legs, each earlier one at the legs its
/// successors' fees rebuild.
pub fn protocol_fees_by_policy<C: ChainTypes>(
    order: &Order<C>,
    policies: &[FeePolicy<C>],
) -> MathResult<Vec<C::Amount>> {
    let base_prices = calculate_custom_prices_from_executed(order);
    let mut current_prices = base_prices;
    let mut total = C::Amount::zero();
    let mut fees = Vec::with_capacity(policies.len());
    for (i, policy) in policies.iter().enumerate().rev() {
        let fee = protocol_fee(order, policy, &current_prices)?;
        fees.push(fee);
        total = total.try_add(fee)?;
        // No policy precedes the first one, so the prices before it are never
        // read.
        if i != 0 {
            current_prices = calculate_custom_prices(order, total, &base_prices)?;
        }
    }
    fees.reverse();
    Ok(fees)
}

/// Surplus of the fill `order` describes over its signed limit, scaled to the
/// executed part, in the surplus token.
pub fn surplus_over_limit<C: ChainTypes>(order: &Order<C>) -> MathResult<C::Amount> {
    surplus_over_limit_price(order, &calculate_custom_prices_from_executed(order))
}

/// Calculate a single protocol fee based on policy type.
fn protocol_fee<C: ChainTypes>(
    order: &Order<C>,
    policy: &FeePolicy<C>,
    custom_prices: &ClearingPrices<C>,
) -> MathResult<C::Amount> {
    match policy {
        FeePolicy::Surplus {
            factor,
            max_volume_factor,
        } => {
            let surplus = surplus_over_limit_price(order, custom_prices)?;
            let surplus_fee = surplus_fee::<C>(surplus, *factor)?;
            let volume_fee = volume_fee(order, custom_prices, *max_volume_factor)?;
            Ok(surplus_fee.min(volume_fee))
        }
        FeePolicy::PriceImprovement {
            factor,
            max_volume_factor,
            quote,
        } => {
            let price_improvement = price_improvement_over_quote(order, custom_prices, quote)?;
            let surplus_fee = surplus_fee::<C>(price_improvement, *factor)?;
            let volume_fee = volume_fee(order, custom_prices, *max_volume_factor)?;
            Ok(surplus_fee.min(volume_fee))
        }
        FeePolicy::Volume { factor } => volume_fee(order, custom_prices, *factor),
    }
}

/// The fee `policy` takes from the fill `order` describes, in the surplus
/// token, with the executed legs as the fee base. The inverse of
/// `protocol_fee`, which recovers the fee from the legs the fill settled at.
pub fn fee_before_settlement<C: ChainTypes>(
    order: &Order<C>,
    policy: &FeePolicy<C>,
) -> MathResult<C::Amount> {
    let volume = match order.side {
        Side::Sell => order.executed_buy,
        Side::Buy => order.executed_sell,
    };
    match policy {
        FeePolicy::Surplus {
            factor,
            max_volume_factor,
        } => {
            let surplus = surplus_at_legs(
                order,
                PriceLimits {
                    sell: order.sell_amount,
                    buy: order.buy_amount,
                },
            )?;
            Ok(surplus
                .try_mul_f64(*factor)?
                .min(volume.try_mul_f64(*max_volume_factor)?))
        }
        FeePolicy::PriceImprovement {
            factor,
            max_volume_factor,
            quote,
        } => {
            let improvement =
                match surplus_at_legs(order, adjust_quote_to_order_limits(order, quote)?) {
                    Err(MathError::Negative) => C::Amount::zero(),
                    improvement => improvement?,
                };
            Ok(improvement
                .try_mul_f64(*factor)?
                .min(volume.try_mul_f64(*max_volume_factor)?))
        }
        FeePolicy::Volume { factor } => volume.try_mul_f64(*factor),
    }
}

/// Takes `policies` from the fill `fill` describes, in list order, each from
/// the legs the one before left: a sell order delivers less buy token, a buy
/// order pulls more sell token. Returns the total fee in the surplus token,
/// which [`protocol_fees_by_policy`] recovers from the settled legs.
///
/// The fees can push the fill past its signed limit. A surplus policy fails
/// on a fill already under it, a price improvement policy takes nothing from
/// a fill under its quote, a volume policy never checks, so the caller checks
/// the limit on the result. On an error `fill` keeps the legs it came with.
pub fn apply_protocol_fees<C: ChainTypes>(
    fill: &mut Order<C>,
    policies: &[FeePolicy<C>],
) -> MathResult<C::Amount> {
    let mut taken = fill.clone();
    let mut total = C::Amount::zero();
    for policy in policies {
        let fee = fee_before_settlement(&taken, policy)?;
        match taken.side {
            Side::Sell => taken.executed_buy = taken.executed_buy.try_sub(fee)?,
            Side::Buy => taken.executed_sell = taken.executed_sell.try_add(fee)?,
        }
        total = total.try_add(fee)?;
    }
    *fill = taken;
    Ok(total)
}

/// Calculate surplus over limit price using custom clearing prices.
fn surplus_over_limit_price<C: ChainTypes>(
    order: &Order<C>,
    prices: &ClearingPrices<C>,
) -> MathResult<C::Amount> {
    surplus_over(
        order,
        prices,
        PriceLimits {
            sell: order.sell_amount,
            buy: order.buy_amount,
        },
    )
}

/// Calculate surplus over arbitrary price limits.
fn surplus_over<C: ChainTypes>(
    order: &Order<C>,
    prices: &ClearingPrices<C>,
    limits: PriceLimits<C>,
) -> MathResult<C::Amount> {
    let executed = match order.side {
        Side::Buy => order.executed_buy,
        Side::Sell => order.executed_sell,
    };

    match order.side {
        Side::Buy => {
            // Scale limit sell to support partially fillable orders
            let limit_sell = limits.sell.try_mul_div_floor(executed, limits.buy)?;

            let sold = executed.try_mul_div_floor(prices.buy, prices.sell)?;

            limit_sell.try_sub(sold)
        }
        Side::Sell => {
            // Scale limit buy to support partially fillable orders (ceiling
            // division)
            let limit_buy = executed.try_mul_div_ceil(limits.buy, limits.sell)?;

            let bought = executed.try_mul_div_ceil(prices.sell, prices.buy)?;

            bought.try_sub(limit_buy)
        }
    }
}

/// Surplus of the fill `order` describes over `limits`, measured on its
/// executed legs. Equals `surplus_over` with the legs as prices, without
/// multiplying the two legs, whose product can exceed the amount type.
fn surplus_at_legs<C: ChainTypes>(
    order: &Order<C>,
    limits: PriceLimits<C>,
) -> MathResult<C::Amount> {
    match order.side {
        Side::Buy => limits
            .sell
            .try_mul_div_floor(order.executed_buy, limits.buy)?
            .try_sub(order.executed_sell),
        Side::Sell => {
            let limit_buy = order
                .executed_sell
                .try_mul_div_ceil(limits.buy, limits.sell)?;
            order.executed_buy.try_sub(limit_buy)
        }
    }
}

/// Calculate price improvement over quote.
///
/// Returns 0 if there's no improvement (instead of error).
fn price_improvement_over_quote<C: ChainTypes>(
    order: &Order<C>,
    prices: &ClearingPrices<C>,
    quote: &Quote<C>,
) -> MathResult<C::Amount> {
    let adjusted_quote = adjust_quote_to_order_limits(order, quote)?;
    match surplus_over(order, prices, adjusted_quote) {
        Ok(surplus) => Ok(surplus),
        Err(MathError::Negative) => Ok(C::Amount::zero()),
        Err(err) => Err(err),
    }
}

/// Adjust quote amounts to be comparable with order limits.
fn adjust_quote_to_order_limits<C: ChainTypes>(
    order: &Order<C>,
    quote: &Quote<C>,
) -> MathResult<PriceLimits<C>> {
    match order.side {
        Side::Sell => {
            // Quote buy amount after fees
            let quote_buy_amount = quote.buy_amount.try_sub(
                quote
                    .fee
                    .try_mul_div_floor(quote.buy_amount, quote.sell_amount)?,
            )?;

            // Scale to order's sell amount
            let scaled_buy_amount =
                quote_buy_amount.try_mul_div_floor(order.sell_amount, quote.sell_amount)?;

            // Use max to handle out-of-market orders
            let buy_amount = order.buy_amount.max(scaled_buy_amount);

            Ok(PriceLimits {
                sell: order.sell_amount,
                buy: buy_amount,
            })
        }
        Side::Buy => {
            // Quote sell amount including fees
            let quote_sell_amount = quote.sell_amount.try_add(quote.fee)?;

            // Scale to order's buy amount
            let scaled_sell_amount =
                quote_sell_amount.try_mul_div_floor(order.buy_amount, quote.buy_amount)?;

            // Use min to handle out-of-market orders
            let sell_amount = order.sell_amount.min(scaled_sell_amount);

            Ok(PriceLimits {
                sell: sell_amount,
                buy: order.buy_amount,
            })
        }
    }
}

/// Calculate surplus fee as a cut of surplus.
///
/// Uses adjusted factor: fee = surplus * factor / (1 - factor)
fn surplus_fee<C: ChainTypes>(surplus: C::Amount, factor: f64) -> MathResult<C::Amount> {
    // Surplus fee is specified as a `factor` from raw surplus (before fee).
    // Since we work with trades that already have the protocol fee applied,
    // we need to calculate the protocol fee using an adjusted factor.
    //
    // fee = surplus_before_fee * factor
    // surplus_after_fee = surplus_before_fee - fee
    // fee = surplus_after_fee * factor / (1 - factor)
    surplus.try_mul_f64(factor / (1.0 - factor))
}

/// Calculate volume fee as a cut of trade volume.
fn volume_fee<C: ChainTypes>(
    order: &Order<C>,
    prices: &ClearingPrices<C>,
    factor: f64,
) -> MathResult<C::Amount> {
    // Volume fee is specified as a factor from raw volume (before fee).
    // We need to calculate using an adjusted factor based on order side.
    //
    // Sell: fee = traded_buy_amount * factor / (1 - factor)
    // Buy:  fee = traded_sell_amount * factor / (1 + factor)

    let executed_in_surplus_token = match order.side {
        Side::Sell => buy_amount(order, prices)?,
        Side::Buy => sell_amount(order, prices)?,
    };

    let adjusted_factor = match order.side {
        Side::Sell => factor / (1.0 - factor),
        Side::Buy => factor / (1.0 + factor),
    };

    executed_in_surplus_token.try_mul_f64(adjusted_factor)
}

/// Calculate custom clearing prices from executed amounts.
///
/// Custom prices are derived from what was actually executed.
fn calculate_custom_prices_from_executed<C: ChainTypes>(order: &Order<C>) -> ClearingPrices<C> {
    ClearingPrices {
        sell: order.executed_buy,
        buy: order.executed_sell,
    }
}

/// Calculate custom clearing prices excluding protocol fees.
///
/// This adjusts prices to reflect the trade without the accumulated fees.
fn calculate_custom_prices<C: ChainTypes>(
    order: &Order<C>,
    protocol_fee: C::Amount,
    prices: &ClearingPrices<C>,
) -> MathResult<ClearingPrices<C>> {
    let sell_amount = sell_amount(order, prices)?;
    let buy_amount = buy_amount(order, prices)?;

    Ok(ClearingPrices {
        sell: match order.side {
            Side::Sell => buy_amount.try_add(protocol_fee)?,
            Side::Buy => buy_amount,
        },
        buy: match order.side {
            Side::Sell => sell_amount,
            Side::Buy => sell_amount.try_sub(protocol_fee)?,
        },
    })
}

/// Calculate effective sell amount (what left user's wallet).
fn sell_amount<C: ChainTypes>(
    order: &Order<C>,
    prices: &ClearingPrices<C>,
) -> MathResult<C::Amount> {
    match order.side {
        Side::Sell => Ok(order.executed_sell),
        Side::Buy => order
            .executed_buy
            .try_mul_div_floor(prices.buy, prices.sell),
    }
}

/// Calculate effective buy amount (what user received).
fn buy_amount<C: ChainTypes>(
    order: &Order<C>,
    prices: &ClearingPrices<C>,
) -> MathResult<C::Amount> {
    match order.side {
        Side::Sell => order
            .executed_sell
            .try_mul_div_ceil(prices.sell, prices.buy),
        Side::Buy => Ok(order.executed_buy),
    }
}
