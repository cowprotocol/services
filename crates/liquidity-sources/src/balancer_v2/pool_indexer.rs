//! HTTP client for the Balancer V2 pools served by the pool-indexer service.
//!
//! Implements [`PoolInitializing`] so the pool-indexer can stand in for the
//! subgraph as the source that seeds the pool registry at start-up. The
//! indexer's `PoolData` response shares the subgraph's wire shape, so the
//! pages deserialize straight into [`PoolData`] and need no remapping.

use {
    super::{
        PoolInitializing,
        models::{PoolData, RegisteredPools},
    },
    anyhow::{Context, Result},
    chain::Chain,
    reqwest::{Client, Url},
    serde::Deserialize,
    std::time::Duration,
};

/// Matches the server-side `MAX_PAGE_LIMIT`.
const LIST_PAGE_SIZE: u64 = 5000;

/// Poll interval while the indexer is still bootstrapping (503).
const READY_POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Cap on the start-up wait for the indexer to serve its first checkpoint.
const READY_TIMEOUT: Duration = Duration::from_secs(60);

pub struct BalancerIndexerClient {
    base_url: Url,
    http: Client,
}

impl BalancerIndexerClient {
    pub fn new(base_url: Url, chain: Chain, http: Client) -> Self {
        let prefix = format!("api/v1/{}/balancer/v2/", chain.as_str());
        Self {
            base_url: url_join(&base_url, &prefix),
            http,
        }
    }

    fn path(&self, suffix: &str) -> Url {
        url_join(&self.base_url, suffix)
    }

    /// `GET /pools?limit=N[&after=cursor]`. `None` means the indexer replied
    /// 503 — still bootstrapping, no checkpoint yet.
    async fn fetch_pools_page(
        &self,
        limit: u64,
        cursor: Option<&str>,
    ) -> Result<Option<PoolsResponse>> {
        let mut url = self.path("pools");
        url.query_pairs_mut()
            .append_pair("limit", &limit.to_string());
        if let Some(c) = cursor {
            url.query_pairs_mut().append_pair("after", c);
        }
        let resp = self.http.get(url).send().await.context("GET /pools")?;
        if resp.status() == reqwest::StatusCode::SERVICE_UNAVAILABLE {
            return Ok(None);
        }
        let page = resp
            .error_for_status()
            .context("/pools HTTP status")?
            .json()
            .await
            .context("/pools body")?;
        Ok(Some(page))
    }

    /// Polls `/pools` until the indexer is past bootstrap (not 503), bounded by
    /// [`READY_TIMEOUT`]. Covers the serve container coming up moments after
    /// the driver; a cold bootstrap is expected to have run before then.
    async fn wait_until_ready(&self) -> Result<()> {
        let deadline = std::time::Instant::now() + READY_TIMEOUT;
        loop {
            if self.fetch_pools_page(1, None).await?.is_some() {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                anyhow::bail!("balancer pool-indexer not ready after {READY_TIMEOUT:?}");
            }
            tracing::debug!("balancer pool-indexer not ready yet (503); waiting");
            tokio::time::sleep(READY_POLL_INTERVAL).await;
        }
    }
}

#[async_trait::async_trait]
impl PoolInitializing for BalancerIndexerClient {
    async fn initialize_pools(&self) -> Result<RegisteredPools> {
        self.wait_until_ready().await?;

        let mut cursor: Option<String> = None;
        let mut pools: Vec<PoolData> = Vec::new();
        let mut fetched_block_number: Option<u64> = None;
        loop {
            let page = self
                .fetch_pools_page(LIST_PAGE_SIZE, cursor.as_deref())
                .await?
                .context("balancer pool-indexer returned 503 after readiness check")?;
            fetched_block_number.get_or_insert(page.block_number);
            pools.extend(page.pools);
            match page.next_cursor {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }

        let registered_pools = RegisteredPools {
            fetched_block_number: fetched_block_number
                .context("balancer pool-indexer returned no pages")?,
            pools,
        };
        tracing::debug!(
            block = %registered_pools.fetched_block_number,
            pools = %registered_pools.pools.len(),
            "initialized registered pools from indexer",
        );
        Ok(registered_pools)
    }
}

/// Wire form of a `/pools` page. `pools` reuses the subgraph's [`PoolData`].
#[derive(Deserialize)]
struct PoolsResponse {
    block_number: u64,
    pools: Vec<PoolData>,
    #[serde(default)]
    next_cursor: Option<String>,
}

/// Joins `path` onto `url` with exactly one slash between them. `Url::join`
/// drops a base's last path segment when it lacks a trailing slash (RFC 3986
/// path resolution), and the operator-supplied indexer URL may omit one.
fn url_join(url: &Url, mut path: &str) -> Url {
    let mut url = url.to_string();
    while url.ends_with('/') {
        url.pop();
    }
    while path.starts_with('/') {
        path = &path[1..];
    }
    Url::parse(&format!("{url}/{path}")).expect("constructed URL is valid")
}
