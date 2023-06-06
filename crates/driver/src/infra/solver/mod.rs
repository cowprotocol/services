use {
    crate::{
        domain::{
            competition::{auction::Auction, solution::Solution, SolverTimeout},
            eth,
            liquidity,
        },
        infra::{self, blockchain::Ethereum, observe},
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

#[derive(Debug, Clone)]
pub struct Slippage {
    pub relative: bigdecimal::BigDecimal,
    pub absolute: Option<eth::Ether>,
}

impl From<String> for Name {
    fn from(inner: String) -> Self {
        Self(inner)
    }
}

impl std::fmt::Display for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Solvers are controlled by the driver. Their job is to search for solutions
/// to auctions. They do this in various ways, often by analyzing different AMMs
/// on the Ethereum blockchain.
#[derive(Debug, Clone)]
pub struct Solver {
    client: reqwest::Client,
    config: Config,
    eth: Ethereum,
    now: infra::time::Now,
}

#[derive(Debug, Clone)]
pub struct Config {
    /// The endpoint of the solver, including the path (commonly "/solve").
    pub endpoint: url::Url,
    pub name: Name,
    /// The acceptable slippage for this solver.
    pub slippage: Slippage,
    /// The private key of this solver, used for settlement submission.
    pub private_key: eth::PrivateKey,
}

impl Solver {
    pub fn new(config: Config, eth: Ethereum, now: infra::time::Now) -> Self {
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
            now,
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
        self.config.private_key.public_address().into()
    }

    /// The private key of this solver.
    pub fn private_key(&self) -> eth::PrivateKey {
        self.config.private_key.clone()
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
            auction, liquidity, timeout, weth, self.now,
        ))
        .unwrap();
        observe::solver_request(&self.config.endpoint, &body);
        let req = self
            .client
            .post(self.config.endpoint.clone())
            .body(body)
            .timeout(timeout.into());
        let res = util::http::send(SOLVER_RESPONSE_MAX_BYTES, req).await;
        observe::solver_response(&self.config.endpoint, res.as_ref().map(String::as_str));
        let res: dto::Solutions = serde_json::from_str(&res?)?;
        let solutions = res.into_domain(auction, liquidity, weth, self.clone())?;

        // Ensure that solution IDs are unique.
        let ids: HashSet<_> = solutions.iter().map(|solution| solution.id).collect();
        if ids.len() != solutions.len() {
            return Err(Error::RepeatedSolutionIds);
        }

        observe::solutions(&solutions);
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
