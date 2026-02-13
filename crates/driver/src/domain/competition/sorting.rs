use {
    crate::{
        domain::competition::{auction::Tokens, order},
        util,
    },
    chrono::Duration,
    shared::domain::eth,
    std::{fmt::Debug, sync::Arc},
};

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub enum SortingKey {
    Float(OrdFloat),
    Timestamp(Option<util::Timestamp>),
    Bool(bool),
}

pub trait SortingStrategy: Send + Sync + Debug {
    fn key(
        &self,
        order: &order::Order,
        tokens: &Tokens,
        solver: &eth::Address,
        now: chrono::DateTime<chrono::Utc>,
    ) -> SortingKey;
}

/// Orders are sorted by their likelihood of being fulfilled, with the most
/// likely orders coming first. See more details in the `likelihood` function
/// docs.
#[derive(Debug)]
pub struct ExternalPrice;
impl SortingStrategy for ExternalPrice {
    fn key(
        &self,
        order: &order::Order,
        tokens: &Tokens,
        _solver: &eth::Address,
        _now: chrono::DateTime<chrono::Utc>,
    ) -> SortingKey {
        // The likelihood that this order will be fulfilled, based on token prices.
        // A larger value means that the order is more likely to be fulfilled.
        // This is used to prioritize orders when solving.
        let chance_to_settle = match (
            tokens.get(&order.buy.token).and_then(|token| token.price),
            tokens.get(&order.sell.token).and_then(|token| token.price),
        ) {
            (Some(buy_price), Some(sell_price)) => {
                let buy = f64::from(buy_price.in_eth(order.buy.amount).0);
                let sell = f64::from(sell_price.in_eth(order.sell.amount).0);
                if buy.is_subnormal() { 0. } else { sell / buy }
            }
            _ => 0.,
        };
        SortingKey::Float(OrdFloat(chance_to_settle))
    }
}

/// We use a wrapper around [f64] to make it sortable
/// which is significantly faster than the
/// [num::BigRational] we used before.
pub struct OrdFloat(f64);
impl PartialOrd for OrdFloat {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for OrdFloat {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}
impl PartialEq for OrdFloat {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for OrdFloat {}

/// Orders are sorted by their creation timestamp, with the most recent orders
/// coming first. If `max_order_age` is set, only orders created within the
/// specified duration will be considered.
#[derive(Debug)]
pub struct CreationTimestamp {
    pub max_order_age: Option<Duration>,
}
impl SortingStrategy for CreationTimestamp {
    fn key(
        &self,
        order: &order::Order,
        _tokens: &Tokens,
        _solver: &eth::Address,
        now: chrono::DateTime<chrono::Utc>,
    ) -> SortingKey {
        SortingKey::Timestamp(match self.max_order_age {
            Some(max_order_age) => {
                let earliest_allowed_creation =
                    u32::try_from((now - max_order_age).timestamp()).unwrap_or(u32::MAX);
                (order.created.0 >= earliest_allowed_creation).then_some(order.created)
            }
            None => Some(order.created),
        })
    }
}

/// Prioritize orders based on whether the current solver provided the winning
/// quote for the order.
#[derive(Debug)]
pub struct OwnQuotes {
    pub max_order_age: Option<Duration>,
}
impl SortingStrategy for OwnQuotes {
    fn key(
        &self,
        order: &order::Order,
        _tokens: &Tokens,
        solver: &eth::Address,
        now: chrono::DateTime<chrono::Utc>,
    ) -> SortingKey {
        let is_order_outdated = self.max_order_age.is_some_and(|max_order_age| {
            let earliest_allowed_creation =
                u32::try_from((now - max_order_age).timestamp()).unwrap_or(u32::MAX);
            order.created.0 < earliest_allowed_creation
        });
        let is_own_quote = order.quote.as_ref().is_some_and(|q| &q.solver == solver);

        SortingKey::Bool(!is_order_outdated && is_own_quote)
    }
}

/// Sort orders based on the provided comparators. Reverse ordering is used to
/// ensure that the most important element comes first.
pub fn sort_orders(
    orders: &mut [order::Order],
    tokens: &Tokens,
    solver: &eth::Address,
    order_comparators: &[Arc<dyn SortingStrategy>],
) {
    let now = chrono::Utc::now();
    orders.sort_by_cached_key(|order| {
        std::cmp::Reverse(
            order_comparators
                .iter()
                .map(|cmp| cmp.key(order, tokens, solver, now))
                .collect::<Vec<_>>(),
        )
    });
}
