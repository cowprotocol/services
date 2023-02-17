//! "Naive" solver implementation.
//!
//! The naive solver is a solver that collects all orders over a single token
//! pair, and matches the excess over a Uniswap V2 pool.

use {
    crate::{
        boundary,
        domain::{auction, liquidity, order, solution},
    },
    std::collections::HashMap,
};

pub struct Naive;

impl Naive {
    /// Solves the specified auction, returning a vector of all possible
    /// solutions.
    pub fn solve(&self, auction: auction::Auction) -> Vec<solution::Solution> {
        let groups = group_by_token_pair(&auction);
        groups
            .values()
            .filter_map(|group| boundary::naive::solve(&group.orders, group.liquidity))
            .collect()
    }
}

#[derive(Debug)]
struct Group<'a> {
    orders: Vec<&'a order::Order>,
    liquidity: &'a liquidity::Liquidity,
    pool: &'a liquidity::constant_product::Pool,
}

type Groups<'a> = HashMap<liquidity::TokenPair, Group<'a>>;

/// Groups an auction by token pairs, where each group contains all orders over
/// the token pair as well as the **deepest** constant product pool.
fn group_by_token_pair(auction: &auction::Auction) -> Groups {
    let mut groups = Groups::new();

    for liquidity in &auction.liquidity {
        let pool = match &liquidity.state {
            liquidity::State::ConstantProduct(pool) => pool,
            _ => continue,
        };

        groups
            .entry(pool.tokens())
            .and_modify(|group| {
                if group.pool.k() < pool.k() {
                    group.liquidity = liquidity;
                    group.pool = pool;
                }
            })
            .or_insert_with(|| Group {
                orders: Vec::new(),
                liquidity,
                pool,
            });
    }

    for order in &auction.orders {
        // the naive solver algorithm is sensitive to 0-amount orders (i.e. they
        // cause panics). Make sure we don't consider them.
        if order.sell.amount.is_zero() || order.buy.amount.is_zero() {
            continue;
        }

        let tokens = match liquidity::TokenPair::new(order.sell.token, order.buy.token) {
            Some(value) => value,
            None => continue,
        };

        groups
            .entry(tokens)
            .and_modify(|group| group.orders.push(order));
    }

    groups.retain(|_, group| !group.orders.is_empty());
    groups
}
