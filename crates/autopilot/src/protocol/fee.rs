//! Protocol fee implementation.
//!
//! The protocol fee is a fee that is defined by the protocol and for each order
//! in the auction we define the way to calculate the protocol fee based on the
//! configuration parameters.

use {
    crate::{
        arguments,
        domain::{self, auction::order::Class},
        driver_model::solve::{fee_policy_to_dto, FeePolicy},
    },
    std::collections::HashMap,
};

pub struct Policies {
    policies: HashMap<domain::OrderUid, Vec<FeePolicy>>,
}

impl Policies {
    pub fn new(auction: &domain::Auction, config: arguments::FeePolicy) -> Self {
        Self {
            policies: auction
                .orders
                .iter()
                .filter_map(|order| {
                    match order.class {
                        Class::Market => None,
                        Class::Liquidity => None,
                        // TODO: https://github.com/cowprotocol/services/issues/2092
                        // skip protocol fee for limit orders with in-market price

                        // TODO: https://github.com/cowprotocol/services/issues/2115
                        // skip protocol fee for TWAP limit orders
                        Class::Limit => Some((order.uid, vec![fee_policy_to_dto(&config)])),
                    }
                })
                .collect(),
        }
    }

    pub fn get(&self, order: &domain::OrderUid) -> Option<Vec<FeePolicy>> {
        self.policies.get(order).cloned()
    }
}
