use {
    crate::domain::eth,
    alloy::{
        network::{EthereumWallet, TxSigner},
        primitives::Log,
        signers::{Signature, Signer, aws::AwsSigner, k256, local::LocalSigner},
        sol,
        transports::{RpcError, TransportErrorKind},
    },
    ethcontract::{
        Account::{self, Kms},
        Address,
        U256,
    },
    pod_sdk::{
        alloy_primitives,
        alloy_sol_types::SolEvent,
        auctions::client::AuctionClient,
        provider::{PodProvider, PodProviderBuilder},
    },
    pod_types::LogFilterBuilder,
    serde::Deserialize,
    serde_with::serde_as,
    std::time::{Duration, UNIX_EPOCH},
    thiserror::Error,
    url::Url,
};

sol! {
    library Time {
        type Timestamp is uint64;
    }

    interface Auction {
        event BidSubmitted(
            uint256 indexed auction_id,
            address indexed bidder,
            Time.Timestamp indexed deadline,
            uint256 value,
            bytes data
        );

        function submitBid(
            uint256 auction_id,
            Time.Timestamp deadline,
            uint256 value,
            bytes memory data
        ) external;
    }
}

fn to_pod_u256(v: U256) -> pod_sdk::U256 {
    let mut bytes = [0u8; 32];
    v.to_big_endian(&mut bytes);
    pod_sdk::U256::from_be_bytes(bytes)
}

fn to_web3_h256(v: pod_sdk::U256) -> web3::types::H256 {
    let bytes: [u8; 32] = v.to_be_bytes();
    web3::types::H256::from_slice(&bytes)
}

fn u64_to_pod_timestamp(v: u64) -> Option<pod_sdk::Timestamp> {
    Some(pod_sdk::Timestamp::from_micros(v as u128))
}

fn make_signer(
    eth_account: ethcontract::Account,
) -> Result<Box<dyn TxSigner<Signature> + Send + Sync>, Error> {
    match eth_account {
        Account::Offline(pk, chain_id) => {
            let bytes = pk.as_ref();
            let key = k256::ecdsa::SigningKey::from_slice(bytes)
                .map_err(|e| Error::FailedToConnect(format!("invalid signing key: {e}")))?;

            let signer = LocalSigner::from(key).with_chain_id(chain_id);
            Ok(Box::new(signer))
        }
        Kms(kms_account, _) => {
            let signer = tokio::runtime::Handle::current().block_on(async {
                AwsSigner::new(
                    kms_account.client().clone(),
                    kms_account.key_id().to_string(),
                    None,
                )
                .await
                .map_err(|e| Error::FailedToConnect(format!("failed to create AwsSigner: {e}")))
            })?;
            Ok(Box::new(signer))
        }
        other => Err(Error::FailedToConnect(format!(
            "unsupported eth_account variant: {other:?}"
        ))),
    }
}

pub struct Pod {
    endpoint: Url,
    pub explorer: Url,
    contract_address: alloy_primitives::Address,
    pod_provider: PodProvider,
}

impl Pod {
    // Initialize a new Pod instance with the given account.
    pub async fn new(
        endpoint: Url,
        explorer: Url,
        contract_address: Address,
    ) -> Result<Self, Error> {
        let pod_provider = PodProviderBuilder::with_recommended_settings()
            .on_url(endpoint.clone())
            .await
            .expect("pod provider should build");
        Ok(Self {
            endpoint,
            explorer,
            contract_address: alloy_primitives::Address::from_slice(contract_address.as_bytes()),
            pod_provider,
        })
    }

    pub async fn bid(
        &self,
        account: Account,
        id: i64,
        deadline: u64,
        value: U256,
        data: &[u8],
    ) -> Result<web3::types::H256, Error> {
        let signer = make_signer(account)?;
        let wallet = EthereumWallet::from(signer);

        // We want to instantiate a new PodProvider with the bider's wallet assigned
        let local_pod_provider = PodProviderBuilder::with_recommended_settings()
            .wallet(wallet)
            .on_url(self.endpoint.clone())
            .await
            .map_err(|e| Error::FailedToConnect(format!("failed to build provider: {e}")))?;

        let auction_client = AuctionClient::new(local_pod_provider, self.contract_address);

        let pod_auction_id =
            pod_sdk::U256::from(u64::try_from(id).map_err(|_| Error::InvalidAuctionId(id))?);
        let pod_auction_value = to_pod_u256(value);

        let receipt = auction_client
            .submit_bid(
                pod_auction_id,
                UNIX_EPOCH + Duration::from_micros(deadline),
                pod_auction_value,
                alloy_primitives::Bytes::copy_from_slice(data).to_vec(),
                Some(0), /* max_fee_per_gas set to 0. Needed to avoid paying gas fees in pod
                          * network. Obviously to be kept only for testing. */
            )
            .await
            .inspect_err(|e| {
                tracing::error!(error = %e, "[pod] submit_bid failed");
            })
            .map_err(|e| Error::SubmitBidFailed(e.to_string()))?;

        tracing::info!(%pod_auction_id, "[pod] submit_bid succeeded");

        Ok(to_web3_h256(receipt.transaction_hash.into()))
    }

    // Waits until the deadline is past perfect ie. no more bids will be approved by
    // the validators.
    pub async fn wait_past_perfect(&self, deadline: u64) -> Result<(), Error> {
        let pod_auction_deadline = u64_to_pod_timestamp(deadline)
            .ok_or_else(|| Error::InvalidDeadline(deadline))?;
        self.pod_provider
            .wait_past_perfect_time(pod_auction_deadline)
            .await
            .inspect_err(|e| {
                tracing::error!(error = %e, "[pod] wait_past_perfect_time failed");
            })
            .map_err(|e| {
                Error::FailedToConnect(format!("failed to wait for past perfect time: {e}"))
            })?;

        Ok(())
    }

    // Returns a list of bids that have enough attestations from the validators to
    // be considered valid. The solver must check which bid has the maximum score to
    // determine the winner.
    pub async fn fetch_bids(
        &self,
        id: i64,
        deadline: u64,
    ) -> Result<Vec<Log<Auction::BidSubmitted>>, Error> {
        let pod_auction_id = pod_sdk::U256::from(id);

        let committee = self.pod_provider.get_committee().await?;
        let quorum_size = committee.quorum_size;

        let filter = LogFilterBuilder::new()
            .address(self.contract_address)
            .event_signature(Auction::BidSubmitted::SIGNATURE_HASH)
            .topic1(pod_auction_id)
            .topic3(pod_sdk::U256::from(deadline))
            .min_attestations(quorum_size as u32)
            .build();

        let verified_logs = self
            .pod_provider
            .get_verifiable_logs(&filter)
            .await
            .inspect_err(|e| {
                tracing::error!(error = %e, "[pod] get_verifiable_logs failed");
            })?;

        verified_logs
            .into_iter()
            .map(|log| {
                Auction::BidSubmitted::decode_log(log.inner.as_ref())
                    .map_err(|e| Error::EventError(e.to_string()))
            })
            .collect()
    }
}

#[serde_as]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    pub endpoint: Url,
    pub explorer: Url,
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
    EventError(String),
    #[error("pod websocket rpc error: {0}")]
    WebsocketError(#[from] RpcError<TransportErrorKind>),
    #[error("pod transaction error: {0}")]
    TransactionError(#[from] ethcontract::errors::ExecutionError),
    #[error("invalid deadline: {0}")]
    InvalidDeadline(u64),
    #[error("invalid auction id: {0}")]
    InvalidAuctionId(i64),
    #[error("pod pending transaction error: {0}")]
    PendingTransactionError(#[from] alloy::providers::PendingTransactionError),
}
