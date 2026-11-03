//! The protocol fee policies the autopilot attaches to an auction order, in
//! the EVM driver's wire shape.

use {super::order_uid::OrderUid, chain_types::solana::Solana, std::collections::HashMap};

pub type FeePolicy = protocol_fees::FeePolicy<Solana>;

/// Each auction order's policies by uid. An order without policies is absent.
pub type FeePolicies = HashMap<OrderUid, Vec<FeePolicy>>;
