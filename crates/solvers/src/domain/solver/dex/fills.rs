use {
    crate::{
        domain::{dex, eth, order},
        util::conv,
    },
    bigdecimal::BigDecimal,
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
    amounts: Mutex<HashMap<order::Uid, CacheEntry>>,
    /// The smallest value in ETH we consider trying a partially fillable order
    /// with. If we move below this threshold we'll restart from 100% fill
    /// amount to not eventually converge at 0.
    smallest_fill: BigDecimal,
}

const ETH: eth::TokenAddress = eth::TokenAddress(eth::H160([0xee; 20]));

impl Fills {
    pub fn new(smallest_fill: eth::Ether) -> Self {
        Self {
            amounts: Default::default(),
            smallest_fill: conv::u256_to_bigdecimal(&smallest_fill.0),
        }
    }

    /// Returns which dex query should be tried for the given order. Takes
    /// information of previous partial fill attempts into account.
    pub fn dex_order(
        &self,
        order: &order::Order,
        prices: &dex::slippage::Prices,
    ) -> Option<dex::Order> {
        if !order.partially_fillable {
            return Some(dex::Order::new(order));
        }

        let (token, total_amount) = match order.side {
            order::Side::Buy => (order.buy.token, order.buy.amount),
            order::Side::Sell => (order.sell.token, order.sell.amount),
        };

        let smallest_fill =
            self.smallest_fill.clone() * prices.0.get(&ETH)? / prices.0.get(&token)?;
        let smallest_fill = conv::bigdecimal_to_u256(&smallest_fill)?;

        let now = Instant::now();

        let amount = match self.amounts.lock().unwrap().entry(order.uid) {
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
            tracing::trace!(?amount, "order no longer worth filling");
            return None;
        }

        let (sell, buy) = match order.side {
            order::Side::Buy => (order.sell, eth::Asset { token, amount }),
            order::Side::Sell => (eth::Asset { token, amount }, order.buy),
        };

        tracing::trace!(?amount, "trying to partially fill order");
        Some(dex::Order::new(&order::Order {
            sell,
            buy,
            ..*order
        }))
    }

    /// Adjusts the next fill amount that should be tried. Always halfes the
    /// last tried amount.
    // TODO: make use of `price_impact` provided by some APIs to get a more optimal
    // next try.
    pub fn reduce_next_try(&self, uid: order::Uid) {
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
    next_amount: eth::U256,
    last_requested: Instant,
}
