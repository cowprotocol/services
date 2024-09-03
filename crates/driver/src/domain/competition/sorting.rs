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
pub enum SortingKey {
    BigRational(num::BigRational),
    Timestamp(Option<util::Timestamp>),
    Bool(bool),
}

pub trait SortingStrategy: Send + Sync {
    fn key(&self, order: &order::Order, tokens: &Tokens, solver: &eth::H160) -> SortingKey;
}

/// Orders are sorted by their likelihood of being fulfilled, with the most
/// likely orders coming first. See more details in the `likelihood` function
/// docs.
pub struct ExternalPrice;
impl SortingStrategy for ExternalPrice {
    fn key(&self, order: &order::Order, tokens: &Tokens, _solver: &eth::H160) -> SortingKey {
        SortingKey::BigRational(order.likelihood(tokens))
    }
}

/// Orders are sorted by their creation timestamp, with the most recent orders
/// coming first. If `max_order_age` is set, only orders created within the
/// specified duration will be considered.
pub struct CreationTimestamp {
    pub max_order_age: Option<Duration>,
}
impl SortingStrategy for CreationTimestamp {
    fn key(&self, order: &order::Order, _tokens: &Tokens, _solver: &eth::H160) -> SortingKey {
        SortingKey::Timestamp(match self.max_order_age {
            Some(max_order_age) => {
                let earliest_allowed_creation =
                    u32::try_from((Utc::now() - max_order_age).timestamp()).unwrap_or(u32::MAX);
                (order.created.0 >= earliest_allowed_creation).then_some(order.created)
            }
            None => Some(order.created),
        })
    }
}

/// Prioritize orders based on whether the current solver provided the winning
/// quote for the order.
pub struct OwnQuotes {
    pub max_order_age: Option<Duration>,
}
impl SortingStrategy for OwnQuotes {
    fn key(&self, order: &order::Order, _tokens: &Tokens, solver: &eth::H160) -> SortingKey {
        let is_order_outdated = self.max_order_age.is_some_and(|max_order_age| {
            let earliest_allowed_creation =
                u32::try_from((Utc::now() - max_order_age).timestamp()).unwrap_or(u32::MAX);
            order.created.0 < earliest_allowed_creation
        });
        let is_own_quote = order.quote.as_ref().is_some_and(|q| &q.solver.0 == solver);

        SortingKey::Bool(!is_order_outdated && is_own_quote)
    }
}

/// Sort orders based on the provided comparators. Reverse ordering is used to
/// ensure that the most important element comes first.
pub fn sort_orders(
    orders: &mut [order::Order],
    tokens: &Tokens,
    solver: &eth::H160,
    order_comparators: &[Arc<dyn SortingStrategy>],
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
