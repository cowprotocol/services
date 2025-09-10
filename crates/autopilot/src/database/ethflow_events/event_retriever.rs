//! A component that listens exclusively for `OrderRefund` events of the ethflow
//! contract.

use {
    ethcontract::{
        H160,
        H256,
        contract::AllEventsBuilder,
        dyns::DynAllEventsBuilder,
        jsonrpc::futures_util::{Stream, TryStreamExt},
    },
    ethrpc::block_stream::RangeInclusive,
    hex_literal::hex,
    shared::{ethrpc::Web3, event_handling::EventRetrieving},
    std::pin::Pin,
    web3::types::Address,
};

const ORDER_REFUND_TOPIC: H256 = H256(hex!(
    "195271068a288191e4b265c641a56b9832919f69e9e7d6c2f31ba40278aeb85a"
));

pub struct EthFlowRefundRetriever {
    web3: Web3,
    addresses: Vec<H160>,
}

impl EthFlowRefundRetriever {
    pub fn new(web3: Web3, addresses: Vec<H160>) -> Self {
        assert!(
            !addresses.is_empty(),
            "EthFlowRefundRetriever must have at least one address to listen to."
        );
        Self { web3, addresses }
    }

    fn get_events(&self) -> DynAllEventsBuilder<contracts::cowswap_eth_flow::Event> {
        let mut events = AllEventsBuilder::new(self.web3.legacy.clone(), H160::default(), None);
        // We want to observe multiple addresses for events.
        events.filter = events.filter.address(self.addresses.clone());
        // Filter out events that we don't want to listen for in the contract. `Self` is
        // designed to only pick up refunding events. Adding a filter also makes
        // the query more efficient.
        events.filter = events.filter.topic0(vec![ORDER_REFUND_TOPIC].into());
        events
    }
}

#[async_trait::async_trait]
impl EventRetrieving for EthFlowRefundRetriever {
    type Event = ethcontract::Event<contracts::cowswap_eth_flow::Event>;

    async fn get_events_by_block_hash(
        &self,
        block_hash: H256,
    ) -> anyhow::Result<Vec<ethcontract::Event<contracts::cowswap_eth_flow::Event>>> {
        Ok(self.get_events().block_hash(block_hash).query().await?)
    }

    async fn get_events_by_block_range(
        &self,
        block_range: &RangeInclusive<u64>,
    ) -> anyhow::Result<
        Pin<
            Box<
                dyn Stream<
                        Item = anyhow::Result<
                            ethcontract::Event<contracts::cowswap_eth_flow::Event>,
                        >,
                    > + Send,
            >,
        >,
    > {
        let stream = self
            .get_events()
            .from_block((*block_range.start()).into())
            .to_block((*block_range.end()).into())
            .block_page_size(500)
            .query_paginated()
            .await?
            .map_err(anyhow::Error::from);

        Ok(Box::pin(stream))
    }

    fn address(&self) -> Vec<Address> {
        self.get_events().filter.address
    }
}
