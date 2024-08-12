use {
    crate::{
        domain::{
            competition::{auction::Tokens, order},
            eth,
        },
        util,
    },
    chrono::{Duration, Utc},
    std::sync::Arc,
};

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub enum SortingKeyType {
    Int(i32),
    BigRational(num::BigRational),
    Timestamp(Option<util::Timestamp>),
    Bool(bool),
}

pub trait SortingKey: Send + Sync {
    fn key(&self, order: &order::Order, tokens: &Tokens, solver: &eth::H160) -> SortingKeyType;
}

/// Prioritize orders by their class: market orders -> limit orders ->
/// liquidity.
///
/// Market orders are preferred over limit orders, as the expectation is that
/// they should be immediately fulfillable. Liquidity orders come last, as they
/// are the most niche and rarely used.
pub struct OrderClass;
impl SortingKey for OrderClass {
    fn key(&self, order: &order::Order, _tokens: &Tokens, _solver: &eth::H160) -> SortingKeyType {
        SortingKeyType::Int(match order.kind {
            order::Kind::Market => 2,
            order::Kind::Limit { .. } => 1,
            order::Kind::Liquidity => 0,
        })
    }
}

/// Orders are sorted by their likelihood of being fulfilled, with the most
/// likely orders coming first. See more details in the `likelihood` function
/// docs.
pub struct ExternalPrice;
impl SortingKey for ExternalPrice {
    fn key(&self, order: &order::Order, tokens: &Tokens, _solver: &eth::H160) -> SortingKeyType {
        SortingKeyType::BigRational(order.likelihood(tokens))
    }
}

/// Orders are sorted by their creation timestamp, with the most recent orders
/// coming first. If `max_order_age` is set, only orders created within the
/// specified duration will be considered.
pub struct CreationTimestamp {
    pub max_order_age: Option<Duration>,
}
impl SortingKey for CreationTimestamp {
    fn key(&self, order: &order::Order, _tokens: &Tokens, _solver: &eth::H160) -> SortingKeyType {
        SortingKeyType::Timestamp(match self.max_order_age {
            Some(max_order_age) => {
                let earliest_allowed_creation =
                    u32::try_from((Utc::now() - max_order_age).timestamp()).unwrap_or(u32::MAX);
                (order.created.0 > earliest_allowed_creation).then_some(order.created)
            }
            None => Some(order.created),
        })
    }
}

/// Prioritize orders based on whether the current solver provided the winning
/// quote for the order.
pub struct OwnQuotes;
impl SortingKey for OwnQuotes {
    fn key(&self, order: &order::Order, _tokens: &Tokens, solver: &eth::H160) -> SortingKeyType {
        SortingKeyType::Bool(order.quote.as_ref().is_some_and(|q| &q.solver.0 == solver))
    }
}

/// Sort orders based on the provided comparators.
pub fn sort_orders(
    orders: &mut [order::Order],
    tokens: &Tokens,
    solver: &eth::H160,
    order_comparators: &[Arc<dyn SortingKey>],
) {
    orders.sort_by_cached_key(|order| {
        std::cmp::Reverse(
            order_comparators
                .iter()
                .map(|cmp| cmp.key(order, tokens, solver))
                .collect::<Vec<_>>(),
        )
    });
}
