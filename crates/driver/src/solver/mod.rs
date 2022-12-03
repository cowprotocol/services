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

/// Solvers are controlled by the driver. Their job is to search for solutions
/// to auctions. They do this in various ways, often by analyzing different AMMs
/// on the Ethereum blockchain.
pub struct Solver {
    url: reqwest::Url,
    client: reqwest::Client,
    /// Used for building the instance name to send to the solver.
    network_name: eth::NetworkName,
    /// Used for building the instance name to send to the solver.
    chain_id: eth::ChainId,
}

const MAX_NR_EXEC_ORDERS: &str = "100";
const SOLVER_RESPONSE_MAX_BYTES: usize = 10_000_000;

// TODO max_nr_exec_orders is always set to 100, so that can be a const for now
// TODO For now the API key seems to never be set
// TODO Ask about Objective - I think this is not needed anymore

// TODO From the SolverConfig, it seems like only the
// use_internal_buffers and objective fields are really used

impl Solver {
    pub fn new(url: reqwest::Url, network_name: eth::NetworkName, chain_id: eth::ChainId) -> Self {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        headers.insert(reqwest::header::ACCEPT, "application/json".parse().unwrap());
        // TODO(#907) Also add an auth header
        Self {
            url,
            client: reqwest::ClientBuilder::new()
                .default_headers(headers)
                .build()
                .unwrap(),
            network_name,
            chain_id,
        }
    }

    pub async fn solve(&self, auction: Auction) -> Result<Solution, Error> {
        // TODO Ask about all the `config` stuff in DefaultHttpSolverApi, what is every
        // field for exactly?
        // TODO Respect auction deadline, leave a buffer of one second like
        // DefaultHttpSolverApi does
        let mut url = self.url.join("solve").unwrap();
        let time_limit = auction.deadline.solver_time_limit()?;
        url.query_pairs_mut()
            .append_pair("auction_id", &auction.id.0.to_string())
            .append_pair("instance_name", &self.instance_name(auction.id))
            .append_pair("time_limit", &time_limit.as_secs().to_string())
            .append_pair("max_nr_exec_orders", MAX_NR_EXEC_ORDERS);
        let body = serde_json::to_string(&dto::Auction::from(auction)).unwrap();
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
            self.network_name.0, self.chain_id.0, auction_id.0
        )
        .replace(&[' ', '/'], "_")
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
}
