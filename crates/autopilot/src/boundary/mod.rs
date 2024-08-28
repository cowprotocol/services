pub use {
    crate::database::{
        competition::Competition,
        order_events::{store_order_events, OrderEventLabel},
    },
    database,
    model::{
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
        DomainSeparator,
    },
    shared::order_validation::{is_order_outside_market_price, Amounts},
};
use {crate::domain, ethrpc::Web3, std::collections::HashMap, url::Url};

pub mod events;
pub mod order;

/// Builds a web3 client that bufferes requests and sends them in a
/// batch call.
pub fn buffered_web3_client(ethrpc: &Url, ethrpc_args: &shared::ethrpc::Arguments) -> Web3 {
    let http_factory =
        shared::http_client::HttpClientFactory::new(&shared::http_client::Arguments {
            http_timeout: std::time::Duration::from_secs(10),
        });
    shared::ethrpc::web3(ethrpc_args, &http_factory, ethrpc, "base")
}

pub struct SolvableOrders {
    pub orders: Vec<model::order::Order>,
    pub quotes: HashMap<domain::OrderUid, domain::Quote>,
    pub latest_settlement_block: u64,
}
