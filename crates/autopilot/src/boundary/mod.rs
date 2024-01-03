use url::Url;
pub use {
    model::{
        app_data::AppDataHash,
        order::{BuyTokenDestination, Order, OrderClass, OrderKind, OrderUid, SellTokenSource},
        signature::Signature,
    },
    shared::{ethrpc::Web3, order_validation::is_order_outside_market_price},
};

pub mod auction;
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
