//! Protocol fee implementation.
//!
//! The protocol fee is a fee that is defined by the protocol and for each order
//! in the auction we define the way to calculate the protocol fee based on the
//! configuration parameters.

use {
    crate::{
        arguments,
        database::Postgres,
        driver_model::solve::{fee_policy_to_dto, FeePolicy},
        run_loop::is_order_outside_market_price,
    },
    anyhow::{Context, Result},
    model::{
        auction::Auction,
        order::{OrderClass, OrderUid},
    },
    number::conversions::big_decimal_to_u256,
    std::{collections::HashMap, sync::Arc},
};

pub struct PolicyFactory {
    config: arguments::FeePolicy,
    db: Arc<Postgres>,
}

impl PolicyFactory {
    pub async fn build(&self, auction: &Auction) -> Result<Policies> {
        let quotes = self
            .db
            .read_quotes(auction)
            .await
            .context("failed to get quotes")?;
        Ok(Policies::new(
            auction
                .orders
                .iter()
                .filter_map(|order| {
                    match order.metadata.class {
                        OrderClass::Market => None,
                        OrderClass::Liquidity => None,
                        // TODO: https://github.com/cowprotocol/services/issues/2115
                        // skip protocol fee for TWAP limit orders
                        OrderClass::Limit(_) => {
                            let quote = quotes.get(&order.metadata.uid)?;
                            let quote_buy_amount = big_decimal_to_u256(&quote.buy_amount)?;
                            let quote_sell_amount = big_decimal_to_u256(&quote.sell_amount)?;
                            let is_in_money_order = !is_order_outside_market_price(
                                &order.data.sell_amount,
                                &order.data.buy_amount,
                                quote_buy_amount,
                                quote_sell_amount,
                            );
                            if self.config.fee_policy_skip_market_orders && is_in_money_order {
                                return None;
                            }
                            Some((order.metadata.uid, vec![fee_policy_to_dto(&self.config)]))
                        }
                    }
                })
                .collect(),
        ))
    }
}

#[derive(Debug, Default)]
pub struct Policies {
    policies: HashMap<OrderUid, Vec<FeePolicy>>,
}

impl Policies {
    pub fn new(policies: HashMap<OrderUid, Vec<FeePolicy>>) -> Self {
        Self { policies }
    }

    pub fn get(&self, order: &OrderUid) -> Option<&Vec<FeePolicy>> {
        self.policies.get(order)
    }
}
