use {
    crate::{
        domain::{auction, dex, eth, order},
        util::conv,
    },
    alloy::primitives::{U256, U512, ruint::UintTryFrom},
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

const ETH: eth::TokenAddress = eth::TokenAddress(eth::Address::repeat_byte(0xee));

impl Fills {
    pub fn new(smallest_fill: eth::Ether) -> Self {
        Self {
            amounts: Default::default(),
            smallest_fill: conv::u256_to_bigdecimal(&smallest_fill.0),
        }
    }

    /// Returns which dex query should be tried for the given order. Takes
    /// information of previous partial fill attempts into account.
    pub fn dex_order(&self, order: &order::Order, tokens: &auction::Tokens) -> Option<dex::Order> {
        // Do not attempt solving for same sell and buy token
        if order.sell.token == order.buy.token {
            return None;
        }
        if !order.partially_fillable {
            return Some(dex::Order::new(order));
        }

        let (token, total_amount) = match order.side {
            order::Side::Buy => (order.buy.token, order.buy.amount),
            order::Side::Sell => (order.sell.token, order.sell.amount),
        };

        let smallest_fill = self.smallest_fill.clone()
            * conv::ether_to_decimal(&tokens.reference_price(&ETH)?.0)
            / conv::ether_to_decimal(&tokens.reference_price(&token)?.0);
        let smallest_fill = conv::bigdecimal_to_u256(&smallest_fill)?;

        let now = Instant::now();

        let amount = match self.amounts.lock().unwrap().entry(order.uid) {
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

        if amount < smallest_fill || amount.is_zero() {
            tracing::trace!(?amount, "order no longer worth filling");
            return None;
        }

        // Scale amounts according to the limit price and the chosen fill.
        let (sell_amount, buy_amount) = match order.side {
            order::Side::Buy => {
                let sell_amount = U256::uint_try_from(
                    order
                        .sell
                        .amount
                        .widening_mul(amount)
                        .checked_div(U512::from(order.buy.amount))?,
                )
                .unwrap();
                (sell_amount, amount)
            }
            order::Side::Sell => {
                let buy_amount = U256::uint_try_from(
                    order
                        .buy
                        .amount
                        .widening_mul(amount)
                        .checked_div(U512::from(order.sell.amount))?,
                )
                .unwrap();
                (amount, buy_amount)
            }
        };

        tracing::trace!(?amount, "trying to partially fill order");
        Some(dex::Order::new(&order::Order {
            sell: eth::Asset {
                token: order.sell.token,
                amount: sell_amount,
            },
            buy: eth::Asset {
                token: order.buy.token,
                amount: buy_amount,
            },
            ..order.clone()
        }))
    }

    /// Adjusts the next fill amount that should be tried. Always halves the
    /// last tried amount.
    pub fn reduce_next_try(&self, uid: order::Uid) {
        self.amounts.lock().unwrap().entry(uid).and_modify(|entry| {
            entry.next_amount /= U256::from(2);
            tracing::trace!(next_try =? entry.next_amount, "reduced next fill amount");
        });
    }

    /// Adjusts the next fill amount that should be tried. Doubles the amount to
    /// try. This is useful in case the onchain liquidity changed and now
    /// allows for bigger fills.
    pub fn increase_next_try(&self, uid: order::Uid) {
        self.amounts.lock().unwrap().entry(uid).and_modify(|entry| {
            entry.next_amount = entry
                .next_amount
                .checked_mul(U256::from(2))
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
    next_amount: eth::U256,
    total_amount: eth::U256,
    last_requested: Instant,
}
