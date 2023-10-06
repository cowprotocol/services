//! "Naive" solver implementation.
//!
//! The naive solver is a solver that collects all orders over a single token
//! pair, computing how many leftover tokens can't be matched peer-to-peer, and
//! matching that excess over a Uniswap V2 pool. This allows for naive
//! coincidence of wants over a single Uniswap V2 pools.

use {
    crate::{
        boundary,
        domain::{self, auction, liquidity, order, solution},
        infra::config,
    },
    std::collections::HashMap,
};

pub struct Naive {
    /// Parameters used to calculate the revert risk of a solution.
    risk: domain::Risk,
}

impl Naive {
    /// Creates a new naive solver for the specified configuration.
    pub fn new(config: config::naive::Config) -> Self {
        Self { risk: config.risk }
    }

    /// Solves the specified auction, returning a vector of all possible
    /// solutions.
    pub async fn solve(&self, auction: auction::Auction) -> Vec<solution::Solution> {
        let risk = self.risk.clone();
        // Make sure to push the CPU-heavy code to a separate thread in order to
        // not lock up the [`tokio`] runtime and cause it to slow down handling
        // the real async things.
        tokio::task::spawn_blocking(move || {
            let groups = group_by_token_pair(&auction);
            groups
                .values()
                .filter_map(|group| {
                    boundary::naive::solve(&group.orders, group.liquidity, &risk, auction.gas_price)
                })
                .collect()
        })
        .await
        .expect("naive solver unexpected panic")
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
/// the token pair as well as the **deepest** constant product pool (i.e. most
/// liquidity, which translates to a higher `K` value for Uniswap V2 style
/// constant product pools).
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
        // The naive solver algorithm is sensitive to 0-amount orders (i.e. they
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
