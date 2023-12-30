//! Protocol fee implementation.
//!
//! The protocol fee is a fee that is defined by the protocol and for each order
//! we define the way to calculate the protocol fee based on the configuration
//! parameters.

use {
    crate::{
        boundary::{self, Order, OrderClass, OrderUid},
        infra::{self},
    },
    std::collections::HashMap,
};

/// Protocol fee policies with cache being updated on each auction.
#[derive(Debug)]
pub struct Policies {
    config: Config,
    database: infra::Database,
}

impl Policies {
    pub fn new(config: Config, database: infra::Database) -> Self {
        Self { config, database }
    }

    /// Get policies for orders.
    pub async fn get(&self, orders: &[Order]) -> Result<HashMap<OrderUid, Vec<Policy>>, Error> {
        let quotes = self
            .database
            .read_quotes(orders.iter().map(|order| &order.metadata.uid))
            .await?;

        Ok(orders
            .iter()
            .filter_map(|order| match order.metadata.class {
                OrderClass::Market => None,
                OrderClass::Liquidity => None,
                OrderClass::Limit(_) => match quotes.get(&order.metadata.uid) {
                    Some(quote) => {
                        let is_market_order = !boundary::is_order_outside_market_price(
                            &order.data.sell_amount,
                            &order.data.buy_amount,
                            &quote.buy_amount,
                            &quote.sell_amount,
                        );
                        if self.config.fee_policy_skip_market_orders && is_market_order {
                            return None;
                        }
                        Some((order.metadata.uid, vec![self.config.policy]))
                    }
                    None => {
                        tracing::warn!(?order.metadata.uid, "quote not found for order");
                        None
                    }
                },
            })
            .collect())
    }
}

#[derive(Debug)]
pub struct Config {
    pub policy: Policy,
    pub fee_policy_skip_market_orders: bool,
}

#[derive(Debug, Copy, Clone)]
pub enum Policy {
    /// If the order receives more than expected (positive deviation from
    /// quoted amounts) pay the protocol a factor of the achieved
    /// improvement. The fee is taken in `sell` token for `buy`
    /// orders and in `buy` token for `sell` orders.
    PriceImprovement {
        /// Factor of price improvement the protocol charges as a fee.
        /// Price improvement is the difference between executed price and
        /// limit price or quoted price (whichever is better)
        ///
        /// E.g. if a user received 2000USDC for 1ETH while having been
        /// quoted 1990USDC, their price improvement is 10USDC.
        /// A factor of 0.5 requires the solver to pay 5USDC to
        /// the protocol for settling this order.
        factor: f64,
        /// Cap protocol fee with a percentage of the order's volume.
        max_volume_factor: f64,
    },
    /// How much of the order's volume should be taken as a protocol fee.
    /// The fee is taken in `sell` token for `sell` orders and in `buy`
    /// token for `buy` orders.
    Volume {
        /// Percentage of the order's volume should be taken as a protocol
        /// fee.
        factor: f64,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read quotes from the database")]
    Db(#[from] infra::database::quotes::Error),
}
