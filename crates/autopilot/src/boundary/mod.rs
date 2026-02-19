pub use {
    crate::database::{
        competition::Competition,
        order_events::{OrderEventLabel, store_order_events},
    },
    database,
    model::{
        DomainSeparator,
        interaction::InteractionData,
        order::{
            BuyTokenDestination,
            EthflowData,
            OnchainOrderData,
            Order,
            OrderClass,
            OrderKind,
            OrderUid,
            SellTokenSource,
        },
        signature::{EcdsaSignature, Signature, SigningScheme},
        solver_competition::SolverCompetitionDB,
    },
    shared::order_validation::{Amounts, is_order_outside_market_price},
};
use {
    crate::domain,
    ethrpc::Web3,
    std::{collections::HashMap, sync::Arc},
    url::Url,
};

pub mod events;
pub mod order;

/// Builds a web3 client based on the ethrpc args config.
pub fn web3_client(ethrpc: &Url, ethrpc_args: &shared::web3::Arguments) -> Web3 {
    shared::web3::web3(ethrpc_args, ethrpc, "base")
}

pub struct SolvableOrders {
    pub orders: HashMap<domain::OrderUid, Arc<model::order::Order>>,
    pub quotes: HashMap<domain::OrderUid, Arc<domain::Quote>>,
    pub latest_settlement_block: u64,
    /// Used as a checkpoint - meaning at this point in time
    /// **at least** the stored orders were present in the system.
    pub fetched_from_db: chrono::DateTime<chrono::Utc>,
}
