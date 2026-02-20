use {
    super::notify,
    crate::{
        domain::{
            competition::{
                auction::{self, Auction},
                order,
                risk_detector,
                solution::{self, Solution},
            },
            eth,
            liquidity,
            time::Remaining,
        },
        infra::{
            self,
            blockchain::Ethereum,
            config::file::FeeHandler,
            persistence::{Persistence, S3},
        },
        util,
    },
    alloy::{
        consensus::SignableTransaction,
        network::TxSigner,
        primitives::Address,
        signers::{Signature, aws::AwsSigner, local::PrivateKeySigner},
    },
    anyhow::{Context, Result},
    derive_more::{From, Into},
    num::BigRational,
    observe::tracing::tracing_headers,
    reqwest::header::HeaderName,
    std::{
        collections::HashMap,
        time::{Duration, Instant},
    },
    thiserror::Error,
    tracing::{Instrument, instrument},
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
pub enum Account {
    PrivateKey(PrivateKeySigner),
    Kms(AwsSigner),
    Address(Address),
}

#[async_trait::async_trait]
impl TxSigner<Signature> for Account {
    fn address(&self) -> Address {
        match self {
            Account::PrivateKey(local_signer) => local_signer.address(),
            Account::Kms(aws_signer) => aws_signer.address(),
            Account::Address(address) => *address,
        }
    }

    async fn sign_transaction(
        &self,
        tx: &mut dyn SignableTransaction<Signature>,
    ) -> alloy::signers::Result<Signature> {
        match self {
            Account::PrivateKey(local_signer) => local_signer.sign_transaction(tx).await,
            Account::Kms(aws_signer) => aws_signer.sign_transaction(tx).await,
            // The address actually can't sign anything but for TxSigner only the Tx matters
            Account::Address(_) => Err(alloy::signers::Error::UnsupportedOperation(
                alloy::signers::UnsupportedSignerOperation::SignHash,
            )),
        }
    }
}

impl From<PrivateKeySigner> for Account {
    fn from(value: PrivateKeySigner) -> Self {
        Self::PrivateKey(value)
    }
}

impl From<AwsSigner> for Account {
    fn from(value: AwsSigner) -> Self {
        Self::Kms(value)
    }
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
    pub account: Account,
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
    pub bad_order_detection: BadOrderDetection,
    /// Max size of the pending settlements queue.
    pub settle_queue_size: usize,
    /// Whether flashloan hints should be sent to the solver.
    pub flashloans_enabled: bool,
    /// Defines at which block the liquidity needs to be fetched on /solve
    /// requests.
    pub fetch_liquidity_at_block: infra::liquidity::AtBlock,
    /// Quote haircut in basis points (0-10000). Applied to solver-reported
    /// economics to make competition bids more conservative. Does not modify
    /// interaction calldata. Default: 0 (no haircut).
    pub haircut_bps: u32,
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
                .tcp_keepalive(Duration::from_secs(60))
                .build()?,
            config,
            eth,
            persistence,
        })
    }

    pub fn bad_order_detection(&self) -> &BadOrderDetection {
        &self.config.bad_order_detection
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
        self.config.account.address()
    }

    /// The account which should be used to sign settlements for this solver.
    pub fn account(&self) -> Account {
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

    pub fn fetch_liquidity_at_block(&self) -> infra::liquidity::AtBlock {
        self.config.fetch_liquidity_at_block.clone()
    }

    /// Quote haircut in basis points (0-10000) for conservative bidding.
    pub fn haircut_bps(&self) -> u32 {
        self.config.haircut_bps
    }

    /// Make a POST request instructing the solver to solve an auction.
    /// Allocates at most `timeout` time for the solving.
    #[instrument(name = "solver_engine", skip_all)]
    pub async fn solve(
        &self,
        auction: &Auction,
        liquidity: &[liquidity::Liquidity],
    ) -> Result<Vec<Solution>, Error> {
        let start = Instant::now();

        let flashloan_hints = self.assemble_flashloan_hints(auction);
        let wrappers = self.assemble_wrappers(auction);

        // Fetch the solutions from the solver.
        let weth = self.eth.contracts().weth_address();
        let auction_dto = dto::auction::new(
            auction,
            liquidity,
            weth,
            self.config.fee_handler,
            self.config.solver_native_token,
            &flashloan_hints,
            &wrappers,
            auction.deadline(self.timeouts()).solvers(),
        );

        if let Some(id) = auction.id() {
            // Only auctions with IDs are real auctions (/quote requests don't have an ID).
            // Only for those it makes sense to archive them and measure the execution time.
            self.persistence.archive_auction(id, &auction_dto);
            ::observe::metrics::metrics().measure_auction_overhead(
                start,
                "driver",
                "serialize_request",
            );
        }

        let body = tokio::task::spawn_blocking(move || {
            // pre-allocate a big enough buffer to avoid re-allocating memory
            // as the request gets serialized
            const BYTES_PER_ORDER: usize = 1_300;
            let mut buffer = Vec::with_capacity(auction_dto.orders.len() * BYTES_PER_ORDER);
            serde_json::to_writer(&mut buffer, &auction_dto)
                .context("serialization failed")
                .map_err(Error::Serialize)?;
            Ok::<_, Error>(bytes::Bytes::from(buffer))
        })
        .await
        .context("serialization task panicked")
        .map_err(Error::Serialize)??;

        let url = shared::url::join(&self.config.endpoint, "solve");
        super::observe::solver_request(&url, &body);
        let timeout = match auction.deadline(self.timeouts()).solvers().remaining() {
            Ok(timeout) => timeout,
            Err(_) => {
                tracing::warn!("auction deadline exceeded before sending request to solver");
                return Ok(Default::default());
            }
        };
        let mut req = self
            .client
            .post(url.clone())
            .body(body)
            .headers(tracing_headers())
            .timeout(timeout);
        if let Some(id) = observe::distributed_tracing::request_id::from_current_span() {
            req = req.header("X-REQUEST-ID", id);
        }
        super::observe::sending_solve_request(
            self.config.name.as_str(),
            timeout,
            auction.id().is_none(),
        );
        let started_at = std::time::Instant::now();
        let res = util::http::send(self.config.response_size_limit_max_bytes, req).await;
        super::observe::solver_response(
            &url,
            res.as_deref(),
            self.config.name.as_str(),
            started_at.elapsed(),
            auction.id().is_none(),
        );
        let res = res?;
        let res: solvers_dto::solution::Solutions =
            serde_json::from_str(&res).inspect_err(|err| {
                tracing::warn!(res, ?err, "failed to parse solver response");
                self.notify(
                    auction.id(),
                    None,
                    notify::Kind::DeserializationError(format!("Request format invalid: {err}")),
                );
            })?;
        let solutions = dto::Solutions::from(res).into_domain(
            auction,
            liquidity,
            weth,
            self.clone(),
            &flashloan_hints,
        )?;

        super::observe::solutions(&solutions, auction.surplus_capturing_jit_order_owners());
        Ok(solutions)
    }

    fn assemble_flashloan_hints(&self, auction: &Auction) -> HashMap<order::Uid, eth::Flashloan> {
        if !self.config.flashloans_enabled {
            return Default::default();
        }

        auction
            .orders()
            .iter()
            .flat_map(|order| {
                let hint = order.app_data.flashloan()?;
                let flashloan = eth::Flashloan {
                    liquidity_provider: hint.liquidity_provider.into(),
                    protocol_adapter: hint.protocol_adapter.into(),
                    receiver: hint.receiver,
                    token: hint.token.into(),
                    amount: hint.amount.into(),
                };
                Some((order.uid, flashloan))
            })
            .collect()
    }

    fn assemble_wrappers(&self, auction: &Auction) -> dto::auction::WrapperCalls {
        auction
            .orders()
            .iter()
            .filter_map(|order| {
                let wrappers = order.app_data.wrappers();
                if wrappers.is_empty() {
                    return None;
                }
                let wrapper_calls = wrappers
                    .iter()
                    .map(|w| solvers_dto::auction::WrapperCall {
                        address: w.address,
                        data: w.data.clone(),
                        is_omittable: w.is_omittable,
                    })
                    .collect();
                Some((order.uid, wrapper_calls))
            })
            .collect()
    }

    /// Make a fire and forget POST request to notify the solver about an event.
    pub fn notify(
        &self,
        auction_id: Option<auction::Id>,
        solution_id: Option<solution::Id>,
        kind: notify::Kind,
    ) {
        let body = serde_json::to_vec(&dto::notification::new(auction_id, solution_id, kind))
            .unwrap()
            .into();
        let url = shared::url::join(&self.config.endpoint, "notify");
        super::observe::solver_request(&url, &body);
        let mut req = self.client.post(url).body(body).headers(tracing_headers());
        if let Some(id) = observe::distributed_tracing::request_id::from_current_span() {
            req = req.header("X-REQUEST-ID", id);
        }
        let response_size = self.config.response_size_limit_max_bytes;
        let future = async move {
            if let Err(error) = util::http::send(response_size, req).await
                && !matches!(error, util::http::Error::NotOk { code: 404, .. })
            {
                tracing::debug!(?error, "failed to notify solver");
            }
        };
        tokio::task::spawn(future.in_current_span());
    }

    pub fn config(&self) -> &Config {
        &self.config
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
    #[error("serialization failed: {0}")]
    Serialize(#[from] anyhow::Error),
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
pub struct BadOrderDetection {
    /// Tokens that are explicitly allow- or deny-listed.
    pub tokens_supported: HashMap<eth::TokenAddress, risk_detector::Quality>,
    pub enable_simulation_strategy: bool,
    pub enable_metrics_strategy: bool,
    pub metrics_strategy_failure_ratio: f64,
    pub metrics_strategy_required_measurements: u32,
    pub metrics_strategy_log_only: bool,
    pub metrics_strategy_order_freeze_time: Duration,
    pub metrics_strategy_cache_gc_interval: Duration,
    pub metrics_strategy_cache_max_age: Duration,
}
