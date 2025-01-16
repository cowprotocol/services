use {
    crate::domain::eth,
    alloy::{providers::{Provider, ProviderBuilder, RootProvider, WsConnect}, pubsub::{PubSubFrontend, Subscription}, transports::{RpcError, TransportErrorKind}},
    ethcontract::{transaction::ResolveCondition, Account, Address, BlockNumber, Event, Topic, U256},
    futures_util::stream::StreamExt,
    pod_auction::event_data,
    serde::Deserialize,
    serde_with::serde_as,
    std::time::Duration,
    thiserror::Error,
    tokio::time::sleep,
    url::Url,
    web3::{transports::Http, Web3},
};

type PodProvider = RootProvider<PubSubFrontend>;

ethcontract::contract!(
    "crates/contracts/artifacts/PodAuction.json",
    contract = PodAuction
);

#[derive(Debug, Clone)]
pub struct Pod {
    http_endpoint: Url,
    ws_endpoint: Url,
    contract_address: Address,
}

impl Pod {
    // Initialize a new Pod instance with the given account.
    pub async fn new(http_endpoint: Url, ws_endpoint: Url, contract_address: Address) -> Result<Self, Error> {
        Ok(Self {
            http_endpoint,
            ws_endpoint,
            contract_address,
        })
    }

    // Submits the specified bid to pod. The method will wait for the transaction to be finalized or return an error.
    pub async fn bid(
        &self,
        account: Account,
        id: u64,
        deadline: u64,
        value: eth::U256,
        data: &[u8],
    ) -> Result<web3::types::H256, Error> {
        let web3 = Web3::new(Http::new(&self.http_endpoint.to_string())
            .map_err(|e| Error::FailedToConnect(e.to_string()))?);
        let contract = PodAuction::with_deployment_info(&web3, self.contract_address, None);

        let tx_hash = contract
            .submit_bid(
                eth::U256::from(id),
                eth::U256::from(deadline),
                value,
                ethcontract::Bytes(data.into()),
            )
            .from(account.clone())
            .into_inner()
            .resolve(ResolveCondition::Pending) // pod doesn't have blocks we will handle confirmation manually
            .send()
            .await?
            .hash();

        for _ in 0..10 {
            let receipt = web3.eth().transaction_receipt(tx_hash).await?;
            if let Some(receipt) = receipt {
                if receipt.status == Some(web3::types::U64::from(1)) {
                    return Ok(tx_hash);
                } else {
                    return Err(Error::SubmitBidFailed("Transaction failed".to_string()));
                }
            }
            sleep(Duration::from_millis(100)).await;
        }

        return Err(Error::SubmitBidFailed(
            "Transaction receipt timedout".to_string(),
        ));
    }

    // Waits until the deadline is past perfect ie. no more bids will be approved by the validators.
    pub async fn wait_past_perfect(&self, deadline: u64) -> Result<(), Error> {
        let provider = self.ws_provider().await?;
        let sub: Subscription<String> = provider
            .subscribe(serde_json::json!(["pod_pastPerfectTime", deadline]))
            .await
            .map_err(|e| Error::FailedToSubscribe(e.to_string()))?;
        tracing::debug!("waiting for past perfection of auction on pod");
        let mut stream = sub.into_stream();

        while let Some(msg) = stream.next().await {
            tracing::debug!("past perfect time reached {:?}", msg);
            break;
        }

        Ok(())
    }

    // Returns a list of bids that have enough attestations from the validators to be considered valid. The solver must check which bid has the maximum score to determine the winner.
    pub async fn fetch_bids(
        &self,
        id: u64,
        deadline: u64,
    ) -> Result<Vec<Event<event_data::BidSubmitted>>, Error> {
        let web3 = Web3::new(Http::new(&self.http_endpoint.to_string())
            .map_err(|e| Error::FailedToConnect(e.to_string()))?);
        let contract = PodAuction::with_deployment_info(&web3, self.contract_address, None);

        let bids = contract
            .events()
            .bid_submitted()
            .from_block(BlockNumber::Earliest)
            .auction_id(Topic::This(U256::from(id)))
            .deadline(Topic::This(U256::from(deadline)))
            .query()
            .await?;

        // TODO: check that the bidder is on a whitelist of approved solvers

        Ok(bids)
    }

    async fn ws_provider(&self) -> Result<PodProvider, Error> {
        let ws = WsConnect::new(self.ws_endpoint.clone());
        Ok(ProviderBuilder::new().on_ws(ws).await?)
    }
}

#[serde_as]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    pub http_endpoint: Url,
    pub ws_endpoint: Url,
    pub contract_address: eth::H160,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to submit bid: {0}")]
    SubmitBidFailed(String),
    #[error("failed to fetch bids: {0}")]
    FailedToSubscribe(String),
    #[error("failed to connect to pod: {0}")]
    FailedToConnect(String),
    #[error("pod web3 error: {0}")]
    Web3(#[from] web3::Error),
    #[error("pod contract error: {0}")]
    Method(#[from] ethcontract::errors::MethodError),
    #[error("pod event error: {0}")]
    EventError(#[from] ethcontract::errors::EventError),
    #[error("pod websocket rpc error: {0}")]
    WebsocketError(#[from] RpcError<TransportErrorKind>),
    #[error("pod transaction error: {0}")]
    TransactionError(#[from] ethcontract::errors::ExecutionError),
    #[error("invalid deadline: {0}")]
    InvalidDeadline(i64),
    #[error("invalid auction id: {0}")]
    InvalidAuctionId(i64),

}
