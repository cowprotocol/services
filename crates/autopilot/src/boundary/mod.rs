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
use {
    crate::domain,
    ethrpc::Web3,
    model::time::now_in_epoch_seconds,
    number::conversions::u256_to_big_uint,
    std::collections::{HashMap, HashSet},
    url::Url,
};

pub mod events;
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

pub struct SolvableOrders {
    pub orders: HashMap<domain::OrderUid, model::order::Order>,
    pub quotes: HashMap<domain::OrderUid, domain::Quote>,
    pub latest_settlement_block: u64,
}

impl SolvableOrders {
    pub fn combine_with(&self, other: Self) -> Self {
        let mut orders = self.orders.clone();
        let mut quotes = self.quotes.clone();
        for (uid, new_order) in other.orders {
            orders.insert(uid, new_order);
        }
        for (uid, quote) in other.quotes {
            quotes.insert(uid, quote);
        }

        let now = now_in_epoch_seconds();
        orders.retain(|_uid, order| {
            let expired = || {
                order.data.valid_to < now
                    || order
                        .metadata
                        .ethflow_data
                        .as_ref()
                        .is_some_and(|data| data.user_valid_to < now as i64)
            };
            let onchain_error = || {
                order
                    .metadata
                    .onchain_order_data
                    .as_ref()
                    .is_some_and(|data| data.placement_error.is_some())
            };
            let fulfilled = || match order.data.kind {
                OrderKind::Sell => {
                    order.metadata.executed_sell_amount >= u256_to_big_uint(&order.data.sell_amount)
                }
                OrderKind::Buy => {
                    order.metadata.executed_buy_amount >= u256_to_big_uint(&order.data.buy_amount)
                }
            };

            !order.metadata.invalidated && !onchain_error() && !expired() && !fulfilled()
        });

        let order_uids = orders.keys().collect::<HashSet<_>>();
        quotes.retain(|uid, _quote| order_uids.contains(uid));

        Self {
            orders,
            quotes,
            latest_settlement_block: self
                .latest_settlement_block
                .max(other.latest_settlement_block),
        }
    }
}
