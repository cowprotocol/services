use {
    alloy::{network::EthereumWallet, primitives::Address, signers::local::PrivateKeySigner},
    pod_sdk::{auctions::client::AuctionClient, provider::PodProviderBuilder},
    url::Url,
};

/// Pod network configuration for tests
pub struct PodConfig {
    pub endpoint: Url,
    pub auction_contract: Address,
}

impl Default for PodConfig {
    fn default() -> Self {
        Self {
            endpoint: super::config::pod::POD_ENDPOINT.parse().unwrap(),
            auction_contract: super::config::pod::POD_AUCTION_CONTRACT.parse().unwrap(),
        }
    }
}

/// Client for querying pod network state in tests
pub struct PodTestClient {
    client: AuctionClient,
}

/// Information about a bid fetched from pod network
#[derive(Debug, Clone)]
pub struct PodBidInfo {
    pub submission_address: Address,
    pub score: pod_sdk::U256,
    pub data_len: usize,
}

impl PodTestClient {
    /// Create a new pod test client with default configuration
    pub async fn new() -> anyhow::Result<Self> {
        Self::with_config(PodConfig::default()).await
    }

    /// Create a new pod test client with custom configuration
    pub async fn with_config(config: PodConfig) -> anyhow::Result<Self> {
        // Create a dummy signer for read-only operations
        // We only need this to satisfy the provider builder requirements
        let dummy_signer = PrivateKeySigner::random();
        let wallet = EthereumWallet::from(dummy_signer);

        let provider = PodProviderBuilder::with_recommended_settings()
            .wallet(wallet)
            .on_url(config.endpoint)
            .await?;

        let client = AuctionClient::new(provider, config.auction_contract);
        Ok(Self { client })
    }

    /// Fetch all bids for a given auction ID. Should only be called after
    /// the auction deadline has passed.
    pub async fn fetch_bids(&self, auction_id: i64) -> anyhow::Result<Vec<PodBidInfo>> {
        let bids = self
            .client
            .fetch_bids(pod_sdk::U256::from(auction_id as u64))
            .await?;
        Ok(bids
            .into_iter()
            .map(|bid| PodBidInfo {
                submission_address: bid.bidder,
                score: bid.amount,
                data_len: bid.data.len(),
            })
            .collect())
    }
}
