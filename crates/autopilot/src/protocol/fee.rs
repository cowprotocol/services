/// Protocol fee implementation.
use {
    crate::{
        arguments,
        driver_model::solve::{fee_policy_to_dto, FeePolicy},
    },
    model::{
        auction::Auction,
        order::{OrderClass, OrderUid},
    },
    std::collections::HashMap,
};

/// Prepares the fee policies for each order in the auction.
/// Determines if the protocol fee should be applied to the order.
pub fn fee_policies(
    auction: &Auction,
    config: arguments::FeePolicy,
) -> HashMap<OrderUid, Vec<FeePolicy>> {
    auction
        .orders
        .iter()
        .map(|order| {
            let fee_policies = match order.metadata.class {
                OrderClass::Market => vec![],
                OrderClass::Liquidity => vec![],
                // todo https://github.com/cowprotocol/services/issues/2092
                // skip protocol fee for limit orders with in-market price

                // todo https://github.com/cowprotocol/services/issues/2115
                // skip protocol fee for TWAP limit orders
                OrderClass::Limit(_) => vec![fee_policy_to_dto(&config)],
            };
            (order.metadata.uid, fee_policies)
        })
        .collect()
}
