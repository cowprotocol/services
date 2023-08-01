use {
    super::observe,
    crate::{
        domain::{
            competition::{auction::Auction, solution::Solution, SolverTimeout},
            eth,
            liquidity,
        },
        infra::blockchain::Ethereum,
        util,
    },
    std::collections::HashSet,
    thiserror::Error,
};

pub mod dto;

const SOLVER_RESPONSE_MAX_BYTES: usize = 10_000_000;

// TODO At some point I should be checking that the names are unique, I don't
// think I'm doing that.
/// The solver name. The user can configure this to be anything that they like.
/// The name uniquely identifies each solver in case there's more than one of
/// them.
#[derive(Debug, Clone)]
pub struct Name(pub String);

impl Name {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for Name {
    fn from(inner: String) -> Self {
        Self(inner)
    }
}

impl std::fmt::Display for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct Slippage {
    pub relative: bigdecimal::BigDecimal,
    pub absolute: Option<eth::Ether>,
}

/// Solvers are controlled by the driver. Their job is to search for solutions
/// to auctions. They do this in various ways, often by analyzing different AMMs
/// on the Ethereum blockchain.
#[derive(Debug, Clone)]
pub struct Solver {
    client: reqwest::Client,
    config: Config,
    eth: Ethereum,
}

#[derive(Debug, Clone)]
pub struct Config {
    /// The endpoint of the solver, including the path (commonly "/solve").
    pub endpoint: url::Url,
    pub name: Name,
    /// The acceptable slippage for this solver.
    pub slippage: Slippage,
    /// The private key of this solver, used for settlement submission.
    pub account: ethcontract::Account,
}

impl Solver {
    pub fn http_time_buffer() -> chrono::Duration {
        chrono::Duration::milliseconds(500)
    }

    pub fn new(config: Config, eth: Ethereum) -> Self {
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
            eth,
        }
    }

    pub fn name(&self) -> &Name {
        &self.config.name
    }

    /// The slippage configuration of this solver.
    pub fn slippage(&self) -> &Slippage {
        &self.config.slippage
    }

    /// The blockchain address of this solver.
    pub fn address(&self) -> eth::Address {
        self.config.account.address().into()
    }

    /// The account which should be used to sign settlements for this solver.
    pub fn account(&self) -> ethcontract::Account {
        self.config.account.clone()
    }

    /// Make a POST request instructing the solver to solve an auction.
    /// Allocates at most `timeout` time for the solving.
    pub async fn solve(
        &self,
        auction: &Auction,
        liquidity: &[liquidity::Liquidity],
        timeout: SolverTimeout,
    ) -> Result<Vec<Solution>, Error> {
        // Fetch the solutions from the solver.
        let weth = self.eth.contracts().weth_address();
        let body = serde_json::to_string(&dto::Auction::new(
            auction,
            liquidity,
            // Reduce the timeout by a small buffer to account for network latency. Otherwise the
            // HTTP timeout might happen before the solver times out its search algorithm.
            timeout.reduce(Self::http_time_buffer()),
            weth,
        ))
        .unwrap();
        observe::solver_request(self.name(), &self.config.endpoint, &body);
        let req = self
            .client
            .post(self.config.endpoint.clone())
            .body(body)
            .timeout(timeout.duration().to_std().unwrap());
        let res = util::http::send(SOLVER_RESPONSE_MAX_BYTES, req).await;
        observe::solver_response(self.name(), &self.config.endpoint, res.as_deref());
        let res: dto::Solutions = serde_json::from_str(&res?)?;
        let solutions = res.into_domain(auction, liquidity, weth, self.clone())?;

        // Ensure that solution IDs are unique.
        let ids: HashSet<_> = solutions.iter().map(|solution| solution.id()).collect();
        if ids.len() != solutions.len() {
            return Err(Error::RepeatedSolutionIds);
        }

        observe::solutions(self.name(), &solutions);
        Ok(solutions)
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("HTTP error: {0:?}")]
    Http(#[from] util::http::Error),
    #[error("JSON deserialization error: {0:?}")]
    Deserialize(#[from] serde_json::Error),
    #[error("solution ids are not unique")]
    RepeatedSolutionIds,
    #[error("solver dto error: {0}")]
    Dto(#[from] dto::Error),
}
