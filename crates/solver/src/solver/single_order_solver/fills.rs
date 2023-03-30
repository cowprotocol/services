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

impl Fills {
    pub fn new(smallest_fill: U256) -> Self {
        Self {
            amounts: Default::default(),
            smallest_fill: number_conversions::u256_to_big_rational(&smallest_fill),
        }
    }

    /// Returns which dex query should be tried for the given order. Takes
    /// information of previous partial fill attempts into account.
    pub fn order(&self, order: &LimitOrder, prices: &ExternalPrices) -> Option<LimitOrder> {
        if !order.partially_fillable {
            return Some(order.clone());
        }

        let (token, total_amount) = match order.kind {
            OrderKind::Buy => (order.buy_token, order.buy_amount),
            OrderKind::Sell => (order.sell_token, order.sell_amount),
        };

        let smallest_fill = prices.try_get_token_amount(&self.smallest_fill, token)?;
        let smallest_fill = number_conversions::big_rational_to_u256(&smallest_fill).ok()?;
        tracing::trace!(?smallest_fill, "least amount worth filling");

        let now = Instant::now();

        let amount = match self.amounts.lock().unwrap().entry(order.id.order_uid()?) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(CacheEntry {
                    next_amount: total_amount,
                    last_requested: now,
                });
                total_amount
            }
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                let entry = entry.get_mut();
                entry.last_requested = now;

                if entry.next_amount < smallest_fill {
                    tracing::trace!("target fill got too small; starting over");
                    entry.next_amount = total_amount;
                } else if entry.next_amount > total_amount {
                    tracing::trace!("partially filled; adjusting to new total amount");
                    entry.next_amount = total_amount;
                }

                entry.next_amount
            }
        };

        if amount < smallest_fill {
            tracing::trace!(?amount, "order no longer worth filling");
            return None;
        }

        let (sell_amount, buy_amount) = match order.kind {
            OrderKind::Buy => (order.sell_amount, amount),
            OrderKind::Sell => (amount, order.buy_amount),
        };

        tracing::trace!(?amount, "trying to partially fill order");
        Some(LimitOrder {
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
            tracing::trace!(next_try =? entry.next_amount, "adjusted next fill amount");
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
}
