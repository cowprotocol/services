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

const SOLVER_RESPONSE_MAX_BYTES: usize = 10_000_000;

/// The solver name. The user can configure this to be anything that they like.
#[derive(Debug, Clone)]
pub struct Name(pub String);

#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
pub struct Solver {
    client: reqwest::Client,
    config: Config,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub url: reqwest::Url,
    pub name: Name,
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
        let solver_deadline = auction.deadline.for_solver()?;
        let body = serde_json::to_string(&dto::Auction::new(auction, &solver_deadline)).unwrap();
        tracing::trace!(%self.config.url, %body, "sending request to solver");
        let req = self
            .client
            .post(self.config.url.clone())
            .body(body)
            .timeout(solver_deadline.into());
        let res = util::http::send(SOLVER_RESPONSE_MAX_BYTES, req).await;
        tracing::trace!(%self.config.url, ?res, "got response from solver");
        let res: dto::Solution = serde_json::from_str(&res?)?;
        Ok(res.into())
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
