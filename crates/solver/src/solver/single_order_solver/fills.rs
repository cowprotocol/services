use {
    crate::liquidity::LimitOrder,
    ethcontract::U256,
    model::order::{OrderKind, OrderUid},
    num::BigRational,
    shared::external_prices::ExternalPrices,
    std::{
        collections::HashMap,
        sync::Mutex,
        time::{Duration, Instant},
    },
};

/// Manages the search for a fillable amount for all order types but
/// specifically for partially fillable orders.
#[derive(Debug)]
pub struct Fills {
    /// Maps which fill amount should be tried next for a given order. For sell
    /// orders the amount refers to the `sell` asset and for buy orders it
    /// refers to the `buy` asset.
    amounts: Mutex<HashMap<OrderUid, CacheEntry>>,
    /// The smallest value in ETH we consider trying a partially fillable order
    /// with. If we move below this threshold we'll restart from 100% fill
    /// amount to not eventually converge at 0.
    smallest_fill: BigRational,
}

/// The reason `Fills::order` failed.
#[derive(Debug)]
pub enum Error {
    MissingPrice,
    /// The smallest fill can't be represented in the order's token.
    SmallestFillU256Conversion,
    /// The order doesn't have a UID (`LimitOrder`s can be created from non GPv2
    /// orders).
    NoOrderUid,
    /// The resulting amount would be less than `Fills::smallest_fill`.
    LessThanSmallestFill,
    /// One of the order's amounts is 0.
    ZeroAmount,
}

impl Fills {
    pub fn new(smallest_fill: U256) -> Self {
        Self {
            amounts: Default::default(),
            smallest_fill: number::conversions::u256_to_big_rational(&smallest_fill),
        }
    }

    /// Returns which dex query should be tried for the given order. Takes
    /// information of previous partial fill attempts into account.
    pub fn order(&self, order: &LimitOrder, prices: &ExternalPrices) -> Result<LimitOrder, Error> {
        if !order.partially_fillable {
            return Ok(order.clone());
        }

        let (token, total_amount) = match order.kind {
            OrderKind::Buy => (order.buy_token, order.buy_amount),
            OrderKind::Sell => (order.sell_token, order.sell_amount),
        };

        let smallest_fill = prices
            .try_get_token_amount(&self.smallest_fill, token)
            .ok_or(Error::MissingPrice)?;
        let smallest_fill = number::conversions::big_rational_to_u256(&smallest_fill)
            .map_err(|_| Error::SmallestFillU256Conversion)?;

        let now = Instant::now();

        let amount = match self
            .amounts
            .lock()
            .unwrap()
            .entry(order.id.order_uid().ok_or(Error::NoOrderUid)?)
        {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(CacheEntry {
                    next_amount: total_amount,
                    last_requested: now,
                    total_amount,
                });
                total_amount
            }
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                let entry = entry.get_mut();
                entry.last_requested = now;
                entry.total_amount = total_amount;

                if entry.next_amount < smallest_fill {
                    tracing::trace!(
                        ?smallest_fill,
                        target =? entry.next_amount,
                        "target fill got too small; starting over"
                    );
                    entry.next_amount = total_amount;
                } else if entry.next_amount > total_amount {
                    tracing::trace!("partially filled; adjusting to new total amount");
                    entry.next_amount = total_amount;
                }

                entry.next_amount
            }
        };

        if amount < smallest_fill {
            return Err(Error::LessThanSmallestFill);
        }

        // Scale amounts according to the limit price and the chosen fill.
        let (sell_amount, buy_amount) = match order.kind {
            OrderKind::Buy => {
                let sell_amount = order
                    .sell_amount
                    .full_mul(amount)
                    .checked_div(order.buy_amount.into())
                    .ok_or(Error::ZeroAmount)?
                    .try_into()
                    .unwrap();
                (sell_amount, amount)
            }
            OrderKind::Sell => {
                let buy_amount = order
                    .buy_amount
                    .full_mul(amount)
                    .checked_div(order.sell_amount.into())
                    .ok_or(Error::ZeroAmount)?
                    .try_into()
                    .unwrap();
                (amount, buy_amount)
            }
        };

        Ok(LimitOrder {
            sell_amount,
            buy_amount,
            ..order.clone()
        })
    }

    /// Adjusts the next fill amount that should be tried. Always halves the
    /// last tried amount.
    // TODO: make use of `price_impact` provided by some APIs to get a more optimal
    // next try.
    pub fn reduce_next_try(&self, uid: OrderUid) {
        self.amounts.lock().unwrap().entry(uid).and_modify(|entry| {
            entry.next_amount /= 2;
            tracing::trace!(next_try =? entry.next_amount, "decreased next fill amount");
        });
    }

    /// Adjusts the next fill amount that should be tried. Doubles the amount to
    /// try. This is useful in case the onchain liquidity changed and now
    /// allows for bigger fills.
    pub fn increase_next_try(&self, uid: OrderUid) {
        self.amounts.lock().unwrap().entry(uid).and_modify(|entry| {
            entry.next_amount = entry
                .next_amount
                .checked_mul(2.into())
                .unwrap_or(entry.total_amount)
                .min(entry.total_amount);
            tracing::trace!(next_try =? entry.next_amount, "increased next fill amount");
        });
    }

    /// Removes entries that have not been requested for a long time. This
    /// allows us to remove orders that got settled by other solvers which
    /// we are not able to notice.
    pub fn collect_garbage(&self) {
        const MAX_AGE: Duration = Duration::from_secs(60 * 10);
        let now = Instant::now();

        self.amounts
            .lock()
            .unwrap()
            .retain(|_, entry| now.duration_since(entry.last_requested) < MAX_AGE)
    }
}

#[derive(Debug)]
struct CacheEntry {
    next_amount: U256,
    last_requested: Instant,
    total_amount: U256,
}
