use {
    crate::{
        logic::{
            competition::{
                auction::{self, Auction},
                solution::Solution,
            },
            eth,
        },
        util,
    },
    thiserror::Error,
};

mod dto;

const MAX_NR_EXEC_ORDERS: &str = "100";
const SOLVER_RESPONSE_MAX_BYTES: usize = 10_000_000;

/// The solver name. The user can configure this to be anything that they like.
#[derive(Debug, Clone)]
pub struct Name(pub String);

#[derive(Debug)]
pub struct Slippage {
    pub relative: num::BigRational,
    pub absolute: Option<eth::Ether>,
}

impl From<String> for Name {
    fn from(inner: String) -> Self {
        Self(inner)
    }
}

/// Solvers are controlled by the driver. Their job is to search for solutions
/// to auctions. They do this in various ways, often by analyzing different AMMs
/// on the Ethereum blockchain.
#[derive(Debug)]
pub struct Solver {
    client: reqwest::Client,
    config: Config,
}

#[derive(Debug)]
pub struct Config {
    pub url: reqwest::Url,
    pub name: Name,
    // TODO After #831 these might not be necessary? Is that correct?
    /// Used for building the instance name to send to the solver.
    pub network_name: eth::NetworkName,
    /// Used for building the instance name to send to the solver.
    pub chain_id: eth::ChainId,
    /// The acceptable slippage for this solver.
    pub slippage: Slippage,
    /// The account of this solver.
    pub account: eth::Account,
}

impl Solver {
    pub fn new(config: Config) -> Self {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        headers.insert(reqwest::header::ACCEPT, "application/json".parse().unwrap());
        // TODO(#907) Also add an auth header
        Self {
            client: reqwest::ClientBuilder::new()
                .default_headers(headers)
                .build()
                .unwrap(),
            config,
        }
    }

    pub fn name(&self) -> &Name {
        &self.config.name
    }

    pub fn slippage(&self) -> &Slippage {
        &self.config.slippage
    }

    pub fn account(&self) -> eth::Account {
        self.config.account
    }

    pub async fn solve(&self, auction: &Auction) -> Result<Solution, Error> {
        let mut url = self.config.url.join("solve").unwrap();
        let time_limit = auction.deadline.solver_time_limit()?;
        url.query_pairs_mut()
            .append_pair("auction_id", &auction.id.0.to_string())
            .append_pair("instance_name", &self.instance_name(auction.id))
            .append_pair("time_limit", &time_limit.as_secs().to_string())
            .append_pair("max_nr_exec_orders", MAX_NR_EXEC_ORDERS);
        let body = serde_json::to_string(&dto::Auction::new(auction)).unwrap();
        tracing::trace!(%url, %body, "sending request to solver");
        let req = self.client.post(url.clone()).body(body).timeout(time_limit);
        let res = util::http::send(SOLVER_RESPONSE_MAX_BYTES, req).await;
        tracing::trace!(%url, ?res, "got response from solver");
        let res: dto::Solution = serde_json::from_str(&res?)?;
        Ok(res.into())
    }

    fn instance_name(&self, auction_id: auction::Id) -> String {
        let now = chrono::Utc::now();
        format!(
            "{now}_{}_{}_{}",
            self.config.network_name.0, self.config.chain_id.0, auction_id.0
        )
        .replace([' ', '/'], "_")
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("HTTP error: {0:?}")]
    Http(#[from] util::http::Error),
    #[error("JSON deserialization error: {0:?}")]
    Deserialize(#[from] serde_json::Error),
    #[error("the auction deadline was exceeded")]
    DeadlineExceeded(#[from] auction::DeadlineExceeded),
    #[error("settlement encoding error: {0:?}")]
    SettlementEncoding(#[from] anyhow::Error),
}
