use contracts::cowswap_onchain_orders;
use database::ethflow_orders::Refund;
use ethcontract::{contract::AllEventsBuilder, transport::DynTransport, H160, H256};
use hex_literal::hex;
use shared::{
    current_block::RangeInclusive,
    ethrpc::Web3,
    event_handling::{EventRetrieving, EventStoring},
};

use crate::database::{events::bytes_to_order_uid, Postgres};

const ORDER_PLACEMENT_TOPIC: H256 = H256(hex!(
    "cf5f9de2984132265203b5c335b25727702ca77262ff622e136baa7362bf1da9"
));
const ORDER_INVALIDATION_TOPIC: H256 = H256(hex!(
    "b8bad102ac8bbacfef31ff1c906ec6d951c230b4dce750bb0376b812ad35852a"
));
static ALL_VALID_ONCHAIN_ORDER_TOPICS: [H256; 2] =
    [ORDER_PLACEMENT_TOPIC, ORDER_INVALIDATION_TOPIC];

// Note: we use a custom implementation of `EventRetrieving` rather than using the one that is
// automatically derivable from the onchain-order contract. This is because the Rust implementation
// of the onchain-order contract assumes that only events that appear in the ABI can be emitted.
// In this custom implementation, we ignore all events except for those specified by the above
// hardcoded topics (which should correspond to the topics of all avents in the onchain-order
// contract ABI).
pub struct CoWSwapOnchainOrdersContract {
    web3: Web3,
    address: H160,
}

impl CoWSwapOnchainOrdersContract {
    pub fn new(web3: Web3, address: H160) -> Self {
        Self { web3, address }
    }
}

impl EventRetrieving for CoWSwapOnchainOrdersContract {
    type Event = cowswap_onchain_orders::Event;

    fn get_events(&self) -> AllEventsBuilder<DynTransport, Self::Event> {
        let mut events = AllEventsBuilder::new(self.web3.clone(), self.address, None);
        // Filter out events that don't belong to the ABI of `OnchainOrdersContract`. This is done
        // because there could be other unrelated events fired by the contract which should be
        // ignored. Also, it makes the request more efficient, since it needs to return less events.
        events.filter = events
            .filter
            .topic0(ALL_VALID_ONCHAIN_ORDER_TOPICS.to_vec().into());
        events
    }
}

const ORDER_REFUND_TOPIC: H256 = H256(hex!(
    "195271068a288191e4b265c641a56b9832919f69e9e7d6c2f31ba40278aeb85a"
));

pub struct EthFlowContract {
    web3: Web3,
    address: H160,
}

impl EthFlowContract {
    pub fn new(web3: Web3, address: H160) -> Self {
        Self { web3, address }
    }
}

impl EventRetrieving for EthFlowContract {
    type Event = contracts::cowswap_eth_flow::Event;

    fn get_events(&self) -> AllEventsBuilder<DynTransport, Self::Event> {
        let mut events = AllEventsBuilder::new(self.web3.clone(), self.address, None);
        // Filter out events that don't belong to the ABI of `OnchainOrdersContract`. This is done
        // because there could be other unrelated events fired by the contract which should be
        // ignored. Also, it makes the request more efficient, since it needs to return less events.
        events.filter = events.filter.topic0(vec![ORDER_REFUND_TOPIC].into());
        events
    }
}

fn get_refunds(events: Vec<ethcontract::Event<EthFlowEvent>>) -> Result<Vec<Refund>> {
    events
        .into_iter()
        .filter_map(|event| {
            let (tx_hash, block_number) = match event.meta {
                Some(meta) => (meta.transaction_hash, meta.block_number),
                None => return Some(Err(anyhow::anyhow!("event without metadata"))),
            };
            let order_uid = match event.data {
                EthFlowEvent::OrderRefund(event) => event.order_uid,
                _ => return None,
            };
            let order_uid = match bytes_to_order_uid(&order_uid.0) {
                Ok(uid) => uid,
                Err(err) => return Some(Err(err)),
            };
            Some(Ok(Refund {
                order_uid,
                tx_hash: database::byte_array::ByteArray(tx_hash.0),
                block_number,
            }))
        })
        .collect()
}

use anyhow::Result;
type EthFlowEvent = contracts::cowswap_eth_flow::Event;
#[async_trait::async_trait]
impl EventStoring<EthFlowEvent> for Postgres {
    async fn last_event_block(&self) -> Result<u64> {
        let mut ex = self.0.acquire().await?;
        let block = database::ethflow_orders::last_indexed_block(&mut ex).await?;
        Ok(block.unwrap_or_default() as u64)
    }

    async fn append_events(&mut self, events: Vec<ethcontract::Event<EthFlowEvent>>) -> Result<()> {
        let refunds = match get_refunds(events)? {
            refunds if !refunds.is_empty() => refunds,
            _ => return Ok(()),
        };
        let _timer = crate::database::Metrics::get()
            .database_queries
            .with_label_values(&["append_ethflow_refund_events"])
            .start_timer();
        let mut ex = self.0.begin().await?;
        database::ethflow_orders::mark_eth_orders_as_refunded(&mut ex, &refunds).await?;
        ex.commit().await?;
        Ok(())
    }

    async fn replace_events(
        &mut self,
        events: Vec<ethcontract::Event<EthFlowEvent>>,
        range: RangeInclusive<u64>,
    ) -> Result<()> {
        let refunds = get_refunds(events)?;
        let _timer = crate::database::Metrics::get()
            .database_queries
            .with_label_values(&["replace_ethflow_refund_events"])
            .start_timer();
        let mut ex = self.0.begin().await?;
        database::ethflow_orders::delete_refunds(
            &mut ex,
            *range.start() as i64,
            *range.end() as i64,
        )
        .await?;
        database::ethflow_orders::mark_eth_orders_as_refunded(&mut ex, &refunds).await?;
        ex.commit().await?;
        Ok(())
    }
}
