use {
    crate::domain::{dex, eth, order},
    std::{
        collections::HashMap,
        sync::Mutex,
        time::{Duration, Instant},
    },
};

/// Manages the search for a fillable amount for partially fillable orders.
#[derive(Debug, Default)]
pub struct PartialFiller {
    /// Maps which fill amount should be tried next for a given order. For sell
    /// orders the amount refers to the `sell` asset and for buy orders it
    /// refers to the `buy` asset.
    amounts: Mutex<HashMap<order::Uid, CacheEntry>>,
}

impl PartialFiller {
    /// Returns which dex query should be tried for the given order. Takes
    /// information of previous partial fill attempts into account.
    pub fn dex_order(&self, order: &order::Order) -> dex::Order {
        if !order.partially_fillable {
            return dex::Order::new(order);
        }

        let (token, total_amount) = match order.side {
            order::Side::Buy => (order.buy.token, order.buy.amount),
            order::Side::Sell => (order.sell.token, order.sell.amount),
        };
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
                // `total_amount` might be lower than what we wanted to try next if
                // some other solver partially filled the order in the mean time.
                entry.next_amount = entry.next_amount.min(total_amount);
                entry.next_amount
            }
        };

        let (sell, buy) = match order.side {
            order::Side::Buy => (order.sell, eth::Asset { token, amount }),
            order::Side::Sell => (eth::Asset { token, amount }, order.buy),
        };

        dex::Order::new(&order::Order {
            sell,
            buy,
            ..*order
        })
    }

    /// Adjusts the next fill amount that should be tried. Always halfes the
    /// last tried amount.
    // TODO: make use of `price_impact` provided by some APIs to get a more optimal
    // next try.
    pub fn reduce_next_try(&self, uid: order::Uid) {
        self.amounts
            .lock()
            .unwrap()
            .entry(uid)
            .and_modify(|entry| entry.next_amount /= 2);
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

// TODO Figure out current problems
// Don't reduce fillable amount indefinitely and when to reset?
