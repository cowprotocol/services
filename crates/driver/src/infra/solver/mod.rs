use {
    super::notify,
    crate::{
        domain::{
            self,
            competition::{
                auction::{self, Auction},
                order,
                risk_detector,
                solution::{self, Solution},
            },
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
    anyhow::Result,
    derive_more::{From, Into},
    eth_domain_types as eth,
    num::BigRational,
    observe::tracing::distributed::headers::tracing_headers,
    reqwest::header::HeaderName,
    std::{collections::HashMap, time::Duration},
    thiserror::Error,
    tracing::{Instrument, instrument},
};

pub mod dto;
pub mod eip7702;
mod streaming;

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

impl Account {
    /// Sign a hash using the underlying signer. Needed for EIP-7702
    /// authorization signing which requires `Signer::sign_hash` rather than
    /// `TxSigner::sign_transaction`.
    pub async fn sign_hash(
        &self,
        hash: &alloy::primitives::B256,
    ) -> alloy::signers::Result<Signature> {
        use alloy::signers::Signer;
        match self {
            Account::PrivateKey(signer) => signer.sign_hash(hash).await,
            Account::Kms(signer) => signer.sign_hash(hash).await,
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
    /// Additional EOAs for parallel settlement submission via EIP-7702.
    /// When non-empty, these accounts submit txs to the solver EOA (which
    /// delegates to Solver7702Delegate), enabling concurrent submissions.
    pub submission_accounts: Vec<Account>,
    /// Maximum number of solutions the driver proposes to the autopilot per
    /// auction. When 1 (the default), only the best-scoring solution is sent.
    pub max_solutions_to_propose: std::num::NonZeroUsize,
    /// How many solutions the driver is allowed to post-process concurrently.
    pub post_processing_concurrency_limit: std::num::NonZeroUsize,
}

impl Config {
    fn validate(&self) -> Result<()> {
        if self.submission_accounts.is_empty() {
            anyhow::ensure!(
                self.max_solutions_to_propose.get() == 1,
                "solver '{}': max-solutions-to-propose > 1 requires non-empty submission-accounts \
                 (EIP-7702 parallel submission must be enabled)",
                self.name,
            );
            return Ok(());
        }

        anyhow::ensure!(
            self.submission_accounts
                .iter()
                .all(|account| !matches!(account, Account::Address(_))),
            "solver '{}': EIP-7702 submission accounts must be signers; address-only accounts \
             cannot sign delegated settlement transactions",
            self.name,
        );
        anyhow::ensure!(
            !matches!(self.account, Account::Address(_)),
            "solver '{}': main account must be a signer to set up EIP-7702 delegation when \
             submission accounts are configured",
            self.name,
        );

        Ok(())
    }
}

impl Solver {
    pub async fn try_new(config: Config, eth: Ethereum) -> Result<Self> {
        config.validate()?;

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

    /// Additional submission accounts for EIP-7702 parallel settlement.
    pub fn submission_accounts(&self) -> &[Account] {
        &self.config.submission_accounts
    }

    pub fn max_solutions_to_propose(&self) -> usize {
        self.config.max_solutions_to_propose.get()
    }

    /// Make a POST request instructing the solver to solve an auction.
    /// Allocates at most `timeout` time for the solving.
    #[instrument(name = "solver_engine", skip_all)]
    pub async fn solve(
        &self,
        auction: &Auction,
        liquidity: &[liquidity::Liquidity],
    ) -> Result<Vec<Solution>, Error> {
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
            self.config.haircut_bps,
        );

        let url = shared::url::join(&self.config.endpoint, "solve");

        // Real auctions (those with an ID) are archived to S3; quotes aren't, so
        // they skip the gzip capture entirely and just stream the body.
        let archive_id = self
            .persistence
            .archives_enabled()
            .then(|| auction.id())
            .flatten();
        let (body, measurements) = match archive_id {
            // Stream the request body while capturing a gzipped copy for S3, so
            // neither the request nor the archive holds the full JSON at once.
            Some(id) => {
                let (body, compressed, measurements) = streaming::stream_body_and_gzip(auction_dto);
                self.persistence.archive_auction_gzipped(id, compressed);
                (body, measurements)
            }
            None => streaming::stream_body(auction_dto),
        };

        // Record the serialization overhead for real auctions only; quotes go
        // through the same streaming path but would skew the metric. The stream
        // reports the timing once serialization finishes, independently of
        // whether the auction was archived.
        if auction.id().is_some() {
            let solver = self.config.name.clone();
            tokio::spawn(async move {
                if let Ok(measurements) = measurements.await {
                    observe::metrics::metrics().record_auction_overhead(
                        measurements.serialize,
                        "driver",
                        "serialize_request",
                    );
                    super::observe::serialized_solve_request(
                        &solver,
                        measurements.serialize,
                        measurements.total,
                    );
                }
            });
        }

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
        if let Some(id) = observe::tracing::distributed::request_id::from_current_span() {
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
        let res: solvers_dto::solution::SolverResponse =
            serde_json::from_str(&res).inspect_err(|err| {
                tracing::warn!(res, ?err, "failed to parse solver response");
                self.notify(
                    auction.id(),
                    None,
                    notify::Kind::DeserializationError(format!("Request format invalid: {err}")),
                );
            })?;

        match res {
            solvers_dto::solution::SolverResponse::Error { error } => {
                tracing::debug!(?error, "solver returned custom error");
                return Err(Error::CustomError(error));
            }
            solvers_dto::solution::SolverResponse::Solutions { solutions } => {
                let solutions = dto::Solutions::from(solutions).into_domain(
                    auction,
                    liquidity,
                    weth,
                    self.clone(),
                    &flashloan_hints,
                )?;

                super::observe::solutions(&solutions, auction.surplus_capturing_jit_order_owners());
                Ok(solutions)
            }
        }
    }

    fn assemble_flashloan_hints(
        &self,
        auction: &Auction,
    ) -> HashMap<order::Uid, domain::flashloan::Flashloan> {
        if !self.config.flashloans_enabled {
            return Default::default();
        }

        auction
            .orders()
            .iter()
            .flat_map(|order| {
                let hint = order.app_data.flashloan()?;
                let flashloan = domain::flashloan::Flashloan {
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
        let body =
            serde_json::to_string(&dto::notification::new(auction_id, solution_id, kind)).unwrap();
        let url = shared::url::join(&self.config.endpoint, "notify");
        super::observe::solver_request(&url, &body);
        let mut req = self.client.post(url).body(body).headers(tracing_headers());
        if let Some(id) = observe::tracing::distributed::request_id::from_current_span() {
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy::primitives::{address, b256},
        std::num::NonZeroUsize,
    };

    const SOLVER: Address = address!("0000000000000000000000000000000000000001");
    const SUBMITTER: Address = address!("0000000000000000000000000000000000000002");

    fn signer() -> Account {
        Account::PrivateKey(
            PrivateKeySigner::from_bytes(&b256!(
                "59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d"
            ))
            .unwrap(),
        )
    }

    fn config() -> Config {
        Config {
            endpoint: "http://localhost/solve".parse().unwrap(),
            name: Name("solver".to_string()),
            slippage: Slippage {
                relative: BigRational::from_integer(0.into()),
                absolute: None,
            },
            liquidity: Liquidity::Fetch,
            account: Account::Address(SOLVER),
            timeouts: Timeouts {
                http_delay: chrono::Duration::seconds(1),
                solving_share_of_deadline: 1.0.try_into().unwrap(),
            },
            request_headers: Default::default(),
            fee_handler: FeeHandler::Driver,
            quote_using_limit_orders: false,
            merge_solutions: SolutionMerging::Forbidden,
            s3: None,
            solver_native_token: ManageNativeToken {
                wrap_address: false,
                insert_unwraps: false,
            },
            quote_tx_origin: None,
            response_size_limit_max_bytes: 1024,
            bad_order_detection: BadOrderDetection {
                tokens_supported: Default::default(),
                enable_simulation_strategy: false,
                enable_metrics_strategy: false,
                metrics_strategy_failure_ratio: 0.9,
                metrics_strategy_required_measurements: 20,
                metrics_strategy_log_only: true,
                metrics_strategy_order_freeze_time: Duration::ZERO,
                metrics_strategy_cache_gc_interval: Duration::ZERO,
                metrics_strategy_cache_max_age: Duration::ZERO,
            },
            settle_queue_size: 0,
            flashloans_enabled: false,
            fetch_liquidity_at_block: infra::liquidity::AtBlock::Latest,
            haircut_bps: 0,
            submission_accounts: vec![],
            max_solutions_to_propose: NonZeroUsize::new(1).unwrap(),
            post_processing_concurrency_limit: NonZeroUsize::MAX,
        }
    }

    #[test]
    fn rejects_multiple_proposed_solutions_without_submission_accounts() {
        let mut config = config();
        config.max_solutions_to_propose = NonZeroUsize::new(2).unwrap();

        let err = config.validate().unwrap_err();

        assert!(err.to_string().contains("requires non-empty"));
    }

    #[test]
    fn rejects_read_only_submission_accounts() {
        let mut config = config();
        config.submission_accounts = vec![Account::Address(SUBMITTER)];

        let err = config.validate().unwrap_err();

        assert!(err.to_string().contains("must be signers"));
    }

    #[test]
    fn rejects_read_only_main_account_with_submission_accounts() {
        let mut config = config();
        config.submission_accounts = vec![signer()];

        let err = config.validate().unwrap_err();

        assert!(err.to_string().contains("main account must be a signer"));
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
    #[error("solver returned custom error: {0:?}")]
    CustomError(solvers_dto::solution::SolverError),
}

impl Error {
    pub fn is_timeout(&self) -> bool {
        match self {
            Self::Http(util::http::Error::Response(err)) => err.is_timeout(),
            _ => false,
        }
    }

    pub fn custom_error(&self) -> Option<&solvers_dto::solution::SolverError> {
        match self {
            Self::CustomError(err) => Some(err),
            _ => None,
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
