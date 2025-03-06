use {
    super::notify,
    crate::{
        domain::{
            competition::{
                auction::{self, Auction},
                bad_tokens,
                solution::{self, Solution},
            },
            eth,
            liquidity,
            time::Remaining,
        },
        infra::{
            blockchain::Ethereum,
            config::file::FeeHandler,
            persistence::{Persistence, S3},
        },
        util,
    },
    anyhow::Result,
    derive_more::{From, Into},
    num::BigRational,
    reqwest::header::HeaderName,
    std::{collections::HashMap, time::Duration},
    tap::TapFallible,
    thiserror::Error,
    tracing::Instrument,
};

pub mod dto;

// TODO At some point I should be checking that the names are unique, I don't
// think I'm doing that.
/// The solver name. The user can configure this to be anything that they like.
/// The name uniquely identifies each solver in case there's more than one of
/// them.
#[derive(Debug, Clone, From, Into)]
pub struct Name(pub String);

impl Name {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct Slippage {
    pub relative: BigRational,
    pub absolute: Option<eth::Ether>,
}

#[derive(Clone, Copy, Debug)]
pub enum Liquidity {
    /// Liquidity should be fetched and included in the auction sent to this
    /// solver.
    Fetch,
    /// The solver does not need liquidity, so fetching can be skipped for this
    /// solver.
    Skip,
}

#[derive(Clone, Copy, Debug)]
pub struct Timeouts {
    /// Maximum time allocated for http request/reponse to propagate through
    /// network.
    pub http_delay: chrono::Duration,
    /// Maximum time allocated for solver engines to return the solutions back
    /// to the driver, in percentage of total driver deadline.
    pub solving_share_of_deadline: util::Percent,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ManageNativeToken {
    /// If true wraps ETH address
    pub wrap_address: bool,
    /// If true inserts unwrap interactions
    pub insert_unwraps: bool,
}

/// Solvers are controlled by the driver. Their job is to search for solutions
/// to auctions. They do this in various ways, often by analyzing different AMMs
/// on the Ethereum blockchain.
#[derive(Debug, Clone)]
pub struct Solver {
    client: reqwest::Client,
    config: Config,
    eth: Ethereum,
    persistence: Persistence,
}

#[derive(Debug, Clone)]
pub struct Config {
    /// The endpoint of the solver, including the path (commonly "/solve").
    pub endpoint: url::Url,
    pub name: Name,
    /// The acceptable slippage for this solver.
    pub slippage: Slippage,
    /// Whether or not liquidity is used by this solver.
    pub liquidity: Liquidity,
    /// The private key of this solver, used for settlement submission.
    pub account: ethcontract::Account,
    /// How much time to spend for each step of the solving and competition.
    pub timeouts: Timeouts,
    /// HTTP headers that should be added to every request.
    pub request_headers: HashMap<String, String>,
    /// Determines whether the `solver` or the `driver` handles the fees
    pub fee_handler: FeeHandler,
    /// Use limit orders for quoting
    /// TODO: Remove once all solvers are moved to use limit orders for quoting
    pub quote_using_limit_orders: bool,
    pub merge_solutions: SolutionMerging,
    /// S3 configuration for storing the auctions in the form they are sent to
    /// the solver engine
    pub s3: Option<S3>,
    /// Whether the native token is wrapped or not when sent to the solvers
    pub solver_native_token: ManageNativeToken,
    /// Which `tx.origin` is required to make quote verification pass.
    pub quote_tx_origin: Option<eth::Address>,
    pub response_size_limit_max_bytes: usize,
    pub bad_token_detection: BadTokenDetection,
    /// Max size of the pending settlements queue.
    pub settle_queue_size: usize,
    /// Whether flashloan hints should be sent to the solver.
    pub flashloans_enabled: bool,
    /// If no lender is specified in flashloan hint, use default one
    pub flashloan_default_lender: eth::Address,
}

impl Solver {
    pub async fn try_new(config: Config, eth: Ethereum) -> Result<Self> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        headers.insert(reqwest::header::ACCEPT, "application/json".parse().unwrap());

        for (key, val) in config.request_headers.iter() {
            let header_name = HeaderName::try_from(key)?;
            headers.insert(header_name, val.parse()?);
        }

        let persistence = Persistence::build(&config).await;

        Ok(Self {
            client: reqwest::ClientBuilder::new()
                .default_headers(headers)
                .build()?,
            config,
            eth,
            persistence,
        })
    }

    pub fn bad_token_detection(&self) -> &BadTokenDetection {
        &self.config.bad_token_detection
    }

    pub fn persistence(&self) -> Persistence {
        self.persistence.clone()
    }

    pub fn name(&self) -> &Name {
        &self.config.name
    }

    /// The slippage configuration of this solver.
    pub fn slippage(&self) -> &Slippage {
        &self.config.slippage
    }

    /// The liquidity configuration of this solver
    pub fn liquidity(&self) -> Liquidity {
        self.config.liquidity
    }

    /// The blockchain address of this solver.
    pub fn address(&self) -> eth::Address {
        self.config.account.address().into()
    }

    /// The account which should be used to sign settlements for this solver.
    pub fn account(&self) -> ethcontract::Account {
        self.config.account.clone()
    }

    /// Timeout configuration for this solver.
    pub fn timeouts(&self) -> Timeouts {
        self.config.timeouts
    }

    /// Use limit orders for quoting instead of market orders
    pub fn quote_using_limit_orders(&self) -> bool {
        self.config.quote_using_limit_orders
    }

    pub fn solution_merging(&self) -> SolutionMerging {
        self.config.merge_solutions
    }

    pub fn solver_native_token(&self) -> ManageNativeToken {
        self.config.solver_native_token
    }

    pub fn quote_tx_origin(&self) -> &Option<eth::Address> {
        &self.config.quote_tx_origin
    }

    pub fn settle_queue_size(&self) -> usize {
        self.config.settle_queue_size
    }

    /// Make a POST request instructing the solver to solve an auction.
    /// Allocates at most `timeout` time for the solving.
    pub async fn solve(
        &self,
        auction: &Auction,
        liquidity: &[liquidity::Liquidity],
    ) -> Result<Vec<Solution>, Error> {
        // Fetch the solutions from the solver.
        let weth = self.eth.contracts().weth_address();
        let auction_dto = dto::auction::new(
            auction,
            liquidity,
            weth,
            self.config.fee_handler,
            self.config.solver_native_token,
            self.config.flashloans_enabled,
            self.config.flashloan_default_lender,
        );
        // Only auctions with IDs are real auctions (/quote requests don't have an ID,
        // and it makes no sense to store them)
        if let Some(id) = auction.id() {
            self.persistence.archive_auction(id, &auction_dto);
        };
        let body = serde_json::to_string(&auction_dto).unwrap();
        let url = shared::url::join(&self.config.endpoint, "solve");
        super::observe::solver_request(&url, &body);
        let timeout = match auction.deadline().solvers().remaining() {
            Ok(timeout) => timeout,
            Err(_) => {
                tracing::warn!("auction deadline exceeded before sending request to solver");
                return Ok(Default::default());
            }
        };
        let mut req = self.client.post(url.clone()).body(body).timeout(timeout);
        if let Some(id) = observe::request_id::from_current_span() {
            req = req.header("X-REQUEST-ID", id);
        }
        let res = util::http::send(self.config.response_size_limit_max_bytes, req).await;
        super::observe::solver_response(&url, res.as_deref());
        let res = res?;
        let res: solvers_dto::solution::Solutions = serde_json::from_str(&res)
            .tap_err(|err| tracing::warn!(res, ?err, "failed to parse solver response"))?;
        let solutions = dto::Solutions::from(res).into_domain(
            auction,
            liquidity,
            weth,
            self.clone(),
            &self.config,
        )?;

        super::observe::solutions(&solutions, auction.surplus_capturing_jit_order_owners());
        Ok(solutions)
    }

    /// Make a fire and forget POST request to notify the solver about an event.
    pub fn notify(
        &self,
        auction_id: Option<auction::Id>,
        solution_id: Option<solution::Id>,
        kind: notify::Kind,
    ) {
        let body =
            serde_json::to_string(&dto::notification::new(auction_id, solution_id, kind)).unwrap();
        let url = shared::url::join(&self.config.endpoint, "notify");
        super::observe::solver_request(&url, &body);
        let mut req = self.client.post(url).body(body);
        if let Some(id) = observe::request_id::from_current_span() {
            req = req.header("X-REQUEST-ID", id);
        }
        let response_size = self.config.response_size_limit_max_bytes;
        let future = async move {
            if let Err(error) = util::http::send(response_size, req).await {
                tracing::warn!(?error, "failed to notify solver");
            }
        };
        tokio::task::spawn(future.in_current_span());
    }
}

/// Controls whether or not the driver is allowed to merge multiple solutions
/// of the same solver to produce an overall better solution.
#[derive(Debug, Clone, Copy)]
pub enum SolutionMerging {
    Allowed {
        max_orders_per_merged_solution: usize,
    },
    Forbidden,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("HTTP error: {0:?}")]
    Http(#[from] util::http::Error),
    #[error("JSON deserialization error: {0:?}")]
    Deserialize(#[from] serde_json::Error),
    #[error("solver dto error: {0}")]
    Dto(#[from] dto::Error),
}

impl Error {
    pub fn is_timeout(&self) -> bool {
        match self {
            Self::Http(util::http::Error::Response(err)) => err.is_timeout(),
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BadTokenDetection {
    /// Tokens that are explicitly allow- or deny-listed.
    pub tokens_supported: HashMap<eth::TokenAddress, bad_tokens::Quality>,
    pub enable_simulation_strategy: bool,
    pub enable_metrics_strategy: bool,
    pub metrics_strategy_failure_ratio: f64,
    pub metrics_strategy_required_measurements: u32,
    pub metrics_strategy_log_only: bool,
    pub metrics_strategy_token_freeze_time: Duration,
}
