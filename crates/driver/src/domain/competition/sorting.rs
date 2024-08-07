use {
    crate::{
        domain::{
            competition::{auction::Tokens, order},
            eth,
        },
        util,
    },
    chrono::{Duration, Utc},
    std::{cmp::Ordering, sync::Arc},
};

pub trait OrderComparator: Send + Sync {
    fn compare(
        &self,
        order_a: &order::Order,
        order_b: &order::Order,
        tokens: &Tokens,
        solver: &eth::H160,
    ) -> Ordering;
}

impl<F> OrderComparator for F
where
    F: Fn(&order::Order, &order::Order, &Tokens, &eth::H160) -> Ordering + Send + Sync,
{
    fn compare(
        &self,
        order_a: &order::Order,
        order_b: &order::Order,
        tokens: &Tokens,
        solver: &eth::H160,
    ) -> Ordering {
        self(order_a, order_b, tokens, solver)
    }
}

pub trait OrderingKey: Send + Sync + 'static {
    type Key: Ord + Send + Sync + 'static;

    fn key(&self, order: &order::Order, tokens: &Tokens, solver: &eth::H160) -> Self::Key;

    /// Returns a comparator that compares two orders based on the key in
    /// reverse order.
    fn comparator(&self) -> Arc<dyn OrderComparator + '_> {
        Arc::new(
            move |a: &order::Order, b: &order::Order, tokens: &Tokens, solver: &eth::H160| {
                self.key(a, tokens, solver)
                    .cmp(&self.key(b, tokens, solver))
                    .reverse()
            },
        )
    }
}

pub struct OrderClass;
impl OrderingKey for OrderClass {
    type Key = i32;

    // Market orders are preferred over limit orders, as the expectation is that
    // they should be immediately fulfillable. Liquidity orders come last, as they
    // are the most niche and rarely used.
    fn key(&self, order: &order::Order, _tokens: &Tokens, _solver: &eth::H160) -> Self::Key {
        match order.kind {
            order::Kind::Market => 2,
            order::Kind::Limit { .. } => 1,
            order::Kind::Liquidity => 0,
        }
    }
}

pub struct ExternalPrice;
impl OrderingKey for ExternalPrice {
    type Key = num::BigRational;

    fn key(&self, order: &order::Order, tokens: &Tokens, _solver: &eth::H160) -> Self::Key {
        order.likelihood(tokens)
    }
}

pub struct CreationTimestamp;
impl CreationTimestamp {
    const THRESHOLD: Duration = Duration::minutes(2);
}
impl OrderingKey for CreationTimestamp {
    type Key = Option<util::Timestamp>;

    fn key(&self, order: &order::Order, _tokens: &Tokens, _solver: &eth::H160) -> Self::Key {
        order.created.filter(|timestamp| {
            timestamp.0
                > u32::try_from((Utc::now() - Self::THRESHOLD).timestamp()).unwrap_or(u32::MAX)
        })
    }
}

pub struct OwnQuotes;
impl OrderingKey for OwnQuotes {
    type Key = bool;

    fn key(&self, order: &order::Order, _tokens: &Tokens, solver: &eth::H160) -> Self::Key {
        order.quote.as_ref().is_some_and(|q| &q.solver.0 == solver)
    }
}

/// Sort orders based on the provided comparators.
pub fn sort_orders(
    orders: &mut [order::Order],
    tokens: &Tokens,
    solver: &eth::H160,
    order_comparators: &Vec<Arc<dyn OrderComparator>>,
) {
    orders.sort_by(|a, b| {
        for cmp in order_comparators {
            let ordering = cmp.compare(a, b, tokens, solver);
            if ordering != Ordering::Equal {
                return ordering;
            }
        }
        Ordering::Equal
    });
}
