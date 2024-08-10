use {
    crate::{
        domain::{
            competition::{auction::Tokens, order},
            eth,
        },
        util,
    },
    chrono::{Duration, Utc},
    std::{cmp::Ordering, collections::HashMap, sync::Arc},
};

type LikelihoodCache = HashMap<order::Uid, num::BigRational>;

pub trait OrderComparator: Send + Sync {
    fn compare(
        &self,
        order_a: &order::Order,
        order_b: &order::Order,
        tokens: &Tokens,
        likelihood_cache: &mut LikelihoodCache,
        solver: &eth::H160,
    ) -> Ordering;
}

impl<F> OrderComparator for F
where
    F: Fn(&order::Order, &order::Order, &Tokens, &mut LikelihoodCache, &eth::H160) -> Ordering
        + Send
        + Sync,
{
    fn compare(
        &self,
        order_a: &order::Order,
        order_b: &order::Order,
        tokens: &Tokens,
        likelihood_cache: &mut LikelihoodCache,
        solver: &eth::H160,
    ) -> Ordering {
        self(order_a, order_b, tokens, likelihood_cache, solver)
    }
}

pub trait OrderingKey: Send + Sync + 'static {
    type Key: Ord + Send + Sync + 'static;

    fn key(
        &self,
        order: &order::Order,
        tokens: &Tokens,
        likelihood_cache: &mut LikelihoodCache,
        solver: &eth::H160,
    ) -> Self::Key;

    /// Returns a comparator that compares two orders based on the key in
    /// reverse order.
    fn into_comparator(self) -> Arc<dyn OrderComparator>
    where
        Self: Sized,
    {
        Arc::new(
            move |a: &order::Order,
                  b: &order::Order,
                  tokens: &Tokens,
                  likelihood_cache: &mut LikelihoodCache,
                  solver: &eth::H160| {
                self.key(a, tokens, likelihood_cache, solver)
                    .cmp(&self.key(b, tokens, likelihood_cache, solver))
                    .reverse()
            },
        )
    }
}

/// Prioritize orders by their class: market orders -> limit orders ->
/// liquidity.
pub struct OrderClass;
impl OrderingKey for OrderClass {
    type Key = i32;

    // Market orders are preferred over limit orders, as the expectation is that
    // they should be immediately fulfillable. Liquidity orders come last, as they
    // are the most niche and rarely used.
    fn key(
        &self,
        order: &order::Order,
        _tokens: &Tokens,
        _likelihood_cache: &mut LikelihoodCache,
        _solver: &eth::H160,
    ) -> Self::Key {
        match order.kind {
            order::Kind::Market => 2,
            order::Kind::Limit { .. } => 1,
            order::Kind::Liquidity => 0,
        }
    }
}

/// Orders are sorted by their likelihood of being fulfilled, with the most
/// likely orders coming first. See more details in the `likelihood` function
/// docs.
pub struct ExternalPrice;
impl OrderingKey for ExternalPrice {
    type Key = num::BigRational;

    fn key(
        &self,
        order: &order::Order,
        tokens: &Tokens,
        likelihood_cache: &mut LikelihoodCache,
        _solver: &eth::H160,
    ) -> Self::Key {
        likelihood_cache
            .entry(order.uid)
            .or_insert_with(|| order.likelihood(tokens))
            .clone()
    }
}

/// Orders are sorted by their creation timestamp, with the most recent orders
/// coming first. If `max_order_age` is set, only orders created within the
/// specified duration will be considered.
pub struct CreationTimestamp {
    pub max_order_age: Option<Duration>,
}
impl OrderingKey for CreationTimestamp {
    type Key = Option<util::Timestamp>;

    fn key(
        &self,
        order: &order::Order,
        _tokens: &Tokens,
        _likelihood_cache: &mut LikelihoodCache,
        _solver: &eth::H160,
    ) -> Self::Key {
        match self.max_order_age {
            Some(max_order_age) => {
                let earliest_allowed_creation =
                    u32::try_from((Utc::now() - max_order_age).timestamp()).unwrap_or(u32::MAX);
                (order.created.0 > earliest_allowed_creation).then_some(order.created)
            }
            None => Some(order.created),
        }
    }
}

/// Prioritize orders based on whether the current solver provided the winning
/// quote for the order.
pub struct OwnQuotes;
impl OrderingKey for OwnQuotes {
    type Key = bool;

    fn key(
        &self,
        order: &order::Order,
        _tokens: &Tokens,
        _likelihood_cache: &mut LikelihoodCache,
        solver: &eth::H160,
    ) -> Self::Key {
        order.quote.as_ref().is_some_and(|q| &q.solver.0 == solver)
    }
}

/// Sort orders based on the provided comparators.
pub fn sort_orders(
    orders: &mut [order::Order],
    tokens: &Tokens,
    solver: &eth::H160,
    order_comparators: &[Arc<dyn OrderComparator>],
) {
    let mut likelihood_cache: LikelihoodCache = HashMap::new();
    orders.sort_by(|a, b| {
        for cmp in order_comparators {
            let ordering = cmp.compare(a, b, tokens, &mut likelihood_cache, solver);
            if ordering != Ordering::Equal {
                return ordering;
            }
        }
        Ordering::Equal
    });
}
