pub use {
    crate::{
        database::{
            competition::Competition,
            order_events::{store_order_events, OrderEventLabel},
        },
        driver_model::{reveal, settle, solve},
    },
    database,
    model::{
        app_data::AppDataHash,
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
        signature::{EcdsaSignature, Signature},
        DomainSeparator,
    },
    shared::order_validation::is_order_outside_market_price,
};
use {ethrpc::Web3, url::Url};

pub mod order;

/// Builds a web3 client that bufferes requests and sends them in a
/// batch call.
pub fn buffered_web3_client(ethrpc: &Url) -> Web3 {
    let ethrpc_args = shared::ethrpc::Arguments {
        ethrpc_max_batch_size: 20,
        ethrpc_max_concurrent_requests: 10,
        ethrpc_batch_delay: Default::default(),
    };
    let http_factory =
        shared::http_client::HttpClientFactory::new(&shared::http_client::Arguments {
            http_timeout: std::time::Duration::from_secs(10),
        });
    shared::ethrpc::web3(&ethrpc_args, &http_factory, ethrpc, "base")
}
