use {
    super::TestAccount,
    crate::{
        nodes::NODE_WS_HOST,
        setup::{
            Contracts,
            OnchainComponents,
            TIMEOUT,
            colocation::{self, SolverEngine},
            wait_for_condition,
        },
    },
    alloy::{
        primitives::{Address, B256, U256},
        providers::ext::AnvilApi,
    },
    app_data::{AppDataDocument, AppDataHash},
    autopilot::{
        config::{Configuration, native_price::NativePriceConfig},
        infra::persistence::dto,
    },
    clap::Parser,
    model::{
        AuctionId,
        order::{CancellationPayload, Order, OrderCreation, OrderUid},
        quote::{NativeTokenPrice, OrderQuoteRequest, OrderQuoteResponse},
        solver_competition,
        solver_competition_v2,
        trade::Trade,
    },
    reqwest::{Client, StatusCode, Url},
    shared::{
        price_estimation::{NativePriceEstimator, NativePriceEstimators},
        web3::Web3,
    },
    sqlx::Connection,
    std::{
        collections::{HashMap, hash_map::Entry},
        ops::DerefMut,
        str::FromStr,
        sync::LazyLock,
        time::Duration,
    },
    tokio::task::JoinHandle,
};

pub const API_HOST: &str = "http://127.0.0.1:8080";
pub const ORDERS_ENDPOINT: &str = "/api/v1/orders";
pub const QUOTING_ENDPOINT: &str = "/api/v1/quote";
pub const ACCOUNT_ENDPOINT: &str = "/api/v1/account";
pub const AUCTION_ENDPOINT: &str = "/api/v1/auction";
pub const TRADES_ENDPOINT: &str = "/api/v1/trades";
pub const VERSION_ENDPOINT: &str = "/api/v1/version";
pub const SOLVER_COMPETITION_ENDPOINT: &str = "/api/v2/solver_competition";
const LOCAL_DB_URL: &str = "postgresql://";
static LOCAL_READ_ONLY_DB_URL: LazyLock<String> = LazyLock::new(|| {
    format!(
        "postgresql://readonly@localhost/{db}",
        db = std::env::var("USER").unwrap()
    )
});

fn order_status_endpoint(uid: &OrderUid) -> String {
    format!("/api/v1/orders/{uid}/status")
}

fn orders_for_tx_endpoint(tx_hash: &B256) -> String {
    format!("/api/v1/transactions/{tx_hash:?}/orders")
}

fn orders_for_owner(owner: &Address, offset: u64, limit: u64) -> String {
    format!("{ACCOUNT_ENDPOINT}/{owner:?}/orders?offset={offset}&limit={limit}")
}

pub struct ServicesBuilder {
    timeout: Duration,
}

impl Default for ServicesBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ServicesBuilder {
    pub fn new() -> Self {
        Self {
            timeout: Duration::from_secs(10),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub async fn build(self, onchain_components: &OnchainComponents) -> Services<'_> {
        Services {
            contracts: onchain_components.contracts(),
            http: Client::builder().timeout(self.timeout).build().unwrap(),
            db: sqlx::PgPool::connect(LOCAL_DB_URL).await.unwrap(),
            web3: onchain_components.web3(),
        }
    }
}

#[derive(Default)]
pub struct ExtraServiceArgs {
    pub api: Vec<String>,
    pub autopilot: Vec<String>,
}

/// Wrapper over offchain services.
/// Exposes various utility methods for tests.
pub struct Services<'a> {
    contracts: &'a Contracts,
    http: Client,
    db: Db,
    web3: &'a Web3,
}

impl<'a> Services<'a> {
    pub async fn new(onchain_components: &'a OnchainComponents) -> Services<'a> {
        Self {
            contracts: onchain_components.contracts(),
            http: Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap(),
            db: sqlx::PgPool::connect(LOCAL_DB_URL).await.unwrap(),
            web3: onchain_components.web3(),
        }
    }

    pub fn builder() -> ServicesBuilder {
        ServicesBuilder::new()
    }

    fn api_autopilot_arguments(&self) -> impl Iterator<Item = String> + use<> {
        [
            "--amount-to-estimate-prices-with=1000000000000000000".to_string(),
            "--block-stream-poll-interval=1s".to_string(),
            format!("--node-ws-url={NODE_WS_HOST}"),
            "--simulation-node-url=http://localhost:8545".to_string(),
            "--native-price-cache-max-age=2s".to_string(),
            format!(
                "--hooks-contract-address={:?}",
                self.contracts.hooks.address()
            ),
        ]
        .into_iter()
    }

    fn autopilot_arguments(&self) -> impl Iterator<Item = String> + use<> {
        self.api_autopilot_arguments()
            .chain(["--quote-timeout=10s".to_string()])
    }

    fn api_autopilot_solver_arguments(&self) -> impl Iterator<Item = String> + use<> {
        [
            "--network-block-interval=1s".to_string(),
            format!(
                "--settlement-contract-address={:?}",
                self.contracts.gp_settlement.address()
            ),
            format!(
                "--balances-contract-address={:?}",
                self.contracts.balances.address()
            ),
            format!(
                "--signatures-contract-address={:?}",
                self.contracts.signatures.address()
            ),
            format!("--native-token-address={:?}", self.contracts.weth.address()),
            format!(
                "--balancer-v2-vault-address={:?}",
                self.contracts.balancer_vault.address()
            ),
        ]
        .into_iter()
    }

    /// Start the autopilot service in a background task.
    /// Optionally specify a solve deadline to use instead of the default 2s.
    /// (note: specifying a larger solve deadline will impact test times as the
    /// driver delays the submission of the solution until shortly before the
    /// deadline in case the solution would start to revert at some point).
    /// Allows to externally control the shutdown of autopilot.
    pub async fn start_autopilot_with_shutdown_controller(
        &self,
        solve_deadline: Option<Duration>,
        extra_args: Vec<String>,
        control: autopilot::shutdown_controller::ShutdownController,
    ) -> JoinHandle<()> {
        let solve_deadline = solve_deadline.unwrap_or(Duration::from_secs(2));
        let ethflow_contracts = self
            .contracts
            .ethflows
            .iter()
            .map(|c| format!("{:?}", c.address()))
            .collect::<Vec<_>>()
            .join(",");

        let args = [
            "autopilot".to_string(),
            "--max-run-loop-delay=100ms".to_string(),
            "--run-loop-native-price-timeout=500ms".to_string(),
            format!("--ethflow-contracts={ethflow_contracts}"),
            "--skip-event-sync=true".to_string(),
            "--api-address=0.0.0.0:12088".to_string(),
            format!("--solve-deadline={solve_deadline:?}"),
        ]
        .into_iter()
        .chain(self.api_autopilot_solver_arguments())
        .chain(self.autopilot_arguments())
        .chain(extra_args)
        .collect();
        let args = ignore_overwritten_cli_params(args);

        let args = autopilot::arguments::CliArguments::try_parse_from(args)
            .map_err(|err| err.to_string())
            .unwrap();
        let config = Configuration {
            native_price_estimation: NativePriceConfig {
                estimators: NativePriceEstimators::new(vec![vec![NativePriceEstimator::driver(
                    "test_quoter".to_string(),
                    Url::from_str("http://localhost:11088/test_solver").unwrap(),
                )]]),
                prefetch_time: Duration::from_millis(500),
                ..Default::default()
            },
            ..Configuration::from_path(&args.config).await.unwrap()
        };
        tracing::info!("Loaded config: {:?}", config);
        let join_handle = tokio::task::spawn(autopilot::run(args, config, control));
        self.wait_until_autopilot_ready().await;

        join_handle
    }

    /// Start the autopilot service in a background task.
    /// Optionally specify a solve deadline to use instead of the default 2s.
    /// (note: specifying a larger solve deadline will impact test times as the
    /// driver delays the submission of the solution until shortly before the
    /// deadline in case the solution would start to revert at some point)
    pub async fn start_autopilot(
        &self,
        solve_deadline: Option<Duration>,
        extra_args: Vec<String>,
    ) -> JoinHandle<()> {
        self.start_autopilot_with_shutdown_controller(
            solve_deadline,
            extra_args,
            autopilot::shutdown_controller::ShutdownController::default(),
        )
        .await
    }

    /// Start the api service in a background tasks.
    /// Wait until the service is responsive.
    pub async fn start_api(&self, extra_args: Vec<String>) {
        let args: Vec<_> = [
            "orderbook".to_string(),
            "--quote-timeout=10s".to_string(),
            "--quote-verification=enforce-when-possible".to_string(),
            "--native-price-estimators=Forwarder|http://localhost:12088".to_string(),
            format!("--db-read-url={}", &*LOCAL_READ_ONLY_DB_URL),
        ]
        .into_iter()
        .chain(self.api_autopilot_solver_arguments())
        .chain(self.api_autopilot_arguments())
        .chain(extra_args)
        .collect();
        let args = ignore_overwritten_cli_params(args);

        let args = orderbook::arguments::Arguments::try_parse_from(args).unwrap();
        tokio::task::spawn(orderbook::run(args));

        Self::wait_for_api_to_come_up().await;
    }

    /// Starts a basic version of the protocol with a single baseline solver.
    pub async fn start_protocol(&self, solver: TestAccount) {
        // HACK: config is required so in the cases where it isn't passed (like the API
        // version test), so we create a dummy one
        let (_config_file, cli_arg) =
            Configuration::test("test_solver", solver.address()).to_cli_args();
        self.start_protocol_with_args(
            ExtraServiceArgs {
                api: Default::default(),
                autopilot: vec![cli_arg],
            },
            solver,
        )
        .await;
    }

    pub async fn start_protocol_with_args(&self, args: ExtraServiceArgs, solver: TestAccount) {
        self.start_protocol_with_args_and_haircut(args, solver, 0)
            .await;
    }

    pub async fn start_protocol_with_args_and_haircut(
        &self,
        args: ExtraServiceArgs,
        solver: TestAccount,
        haircut_bps: u32,
    ) {
        colocation::start_driver(
            self.contracts,
            vec![
                colocation::start_baseline_solver_with_haircut(
                    "test_solver".into(),
                    solver.clone(),
                    *self.contracts.weth.address(),
                    vec![],
                    1,
                    true,
                    haircut_bps,
                )
                .await,
            ],
            colocation::LiquidityProvider::UniswapV2,
            false,
        );

        self.start_autopilot(
            None,
            [
                vec![
                    "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver"
                        .to_string(),
                    "--gas-estimators=http://localhost:11088/gasprice".to_string(),
                ],
                args.autopilot,
            ]
            .concat(),
        )
        .await;
        self.start_api(
            [
                vec![
                    "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver"
                        .to_string(),
                    "--gas-estimators=http://localhost:11088/gasprice".to_string(),
                ],
                args.api,
            ]
            .concat(),
        )
        .await;
    }

    /// Starts a basic version of the protocol with a single external solver.
    /// Optionally starts a baseline solver and uses it for price estimation.
    pub async fn start_protocol_external_solver(
        &self,
        solver: TestAccount,
        solver_endpoint: Option<Url>,
        run_baseline: bool,
    ) {
        let external_solver_endpoint =
            solver_endpoint.unwrap_or("http://localhost:8000/".parse().unwrap());

        let mut solvers = vec![SolverEngine {
            name: "test_solver".into(),
            account: solver.clone(),
            endpoint: external_solver_endpoint,
            base_tokens: vec![],
            merge_solutions: true,
            haircut_bps: 0,
        }];

        // Create TOML config file for the driver
        let (_config_file, config_arg) = Configuration {
            native_price_estimation: {
                if run_baseline {
                    NativePriceConfig {
                        estimators: NativePriceEstimators::new(vec![vec![
                            NativePriceEstimator::driver(
                                "test_quoter".to_string(),
                                Url::from_str("http://localhost:11088/baseline_solver").unwrap(),
                            ),
                            NativePriceEstimator::driver(
                                "test_solver".to_string(),
                                Url::from_str("http://localhost:11088/test_solver").unwrap(),
                            ),
                        ]]),
                        ..Default::default()
                    }
                } else {
                    NativePriceConfig {
                        estimators: NativePriceEstimators::new(vec![vec![
                            NativePriceEstimator::driver(
                                "test_quoter".to_string(),
                                Url::from_str("http://localhost:11088/test_solver").unwrap(),
                            ),
                        ]]),
                        ..Default::default()
                    }
                }
            },
            ..Configuration::test("test_solver", solver.address())
        }
        .to_cli_args();

        let (autopilot_args, api_args) = if run_baseline {
            solvers.push(
                colocation::start_baseline_solver(
                    "baseline_solver".into(),
                    solver.clone(),
                    *self.contracts.weth.address(),
                    vec![],
                    1,
                    true,
                )
                .await,
            );

            // Here we call the baseline_solver "test_quoter" to make the native price
            // estimation use the baseline_solver instead of the test_quoter
            let autopilot_args = vec![
                config_arg.clone(),
                "--price-estimation-drivers=test_quoter|http://localhost:11088/baseline_solver,test_solver|http://localhost:11088/test_solver".to_string(),
            ];
            let api_args = vec![
                "--price-estimation-drivers=test_quoter|http://localhost:11088/baseline_solver,test_solver|http://localhost:11088/test_solver".to_string(),
            ];
            (autopilot_args, api_args)
        } else {
            let autopilot_args = vec![
                config_arg,
                "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver"
                    .to_string(),
            ];

            let api_args = vec![
                "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver"
                    .to_string(),
            ];
            (autopilot_args, api_args)
        };

        colocation::start_driver(
            self.contracts,
            solvers,
            colocation::LiquidityProvider::UniswapV2,
            false,
        );

        self.start_autopilot(Some(Duration::from_secs(11)), autopilot_args)
            .await;
        self.start_api(api_args).await;
    }

    async fn wait_for_api_to_come_up() {
        let is_up = || async {
            reqwest::get(format!("{API_HOST}{VERSION_ENDPOINT}"))
                .await
                .is_ok()
        };

        tracing::info!("Waiting for API to come up.");
        wait_for_condition(TIMEOUT, is_up)
            .await
            .expect("waiting for API timed out");
    }

    async fn wait_until_autopilot_ready(&self) {
        let is_up = || async {
            let mut db = self.db.acquire().await.unwrap();
            const QUERY: &str = "SELECT COUNT(*) FROM auctions";
            let count: i64 = sqlx::query_scalar(QUERY)
                .fetch_one(db.deref_mut())
                .await
                .unwrap();
            self.mint_block().await;
            count > 0
        };
        wait_for_condition(TIMEOUT, is_up)
            .await
            .expect("waiting for autopilot timed out");
    }

    /// Fetches the current auction. Don't use this as a synchronization
    /// mechanism in tests because that is prone to race conditions
    /// which would make tests flaky.
    pub async fn get_auction(&self) -> dto::Auction {
        let response = self
            .http
            .get(format!("{API_HOST}{AUCTION_ENDPOINT}"))
            .send()
            .await
            .unwrap();

        let status = response.status();
        let body = response.text().await.unwrap();

        assert_eq!(status, StatusCode::OK, "{body}");

        serde_json::from_str(&body).unwrap()
    }

    pub async fn get_solver_competition(
        &self,
        hash: B256,
    ) -> Result<solver_competition_v2::Response, StatusCode> {
        let response = self
            .http
            .get(format!(
                "{API_HOST}{SOLVER_COMPETITION_ENDPOINT}/by_tx_hash/{hash:?}"
            ))
            .send()
            .await
            .unwrap();

        let status = response.status();
        let body = response.text().await.unwrap();

        match status {
            StatusCode::OK => Ok(serde_json::from_str(&body).unwrap()),
            code => Err(code),
        }
    }

    pub async fn get_latest_solver_competition(
        &self,
    ) -> Result<solver_competition_v2::Response, StatusCode> {
        let response = self
            .http
            .get(format!("{API_HOST}{SOLVER_COMPETITION_ENDPOINT}/latest"))
            .send()
            .await
            .unwrap();

        let status = response.status();
        let body = response.text().await.unwrap();
        match status {
            StatusCode::OK => Ok(serde_json::from_str(&body).unwrap()),
            code => Err(code),
        }
    }

    pub async fn get_trades(&self, order: &OrderUid) -> Result<Vec<Trade>, StatusCode> {
        let url = format!("{API_HOST}/api/v1/trades?orderUid={order}");
        let response = self.http.get(url).send().await.unwrap();

        let status = response.status();
        let body = response.text().await.unwrap();

        match status {
            StatusCode::OK => Ok(serde_json::from_str(&body).unwrap()),
            code => Err(code),
        }
    }

    /// Create an [`Order`].
    /// If the response status code is not `201`, return the status and the
    /// body.
    pub async fn create_order(
        &self,
        order: &OrderCreation,
    ) -> Result<OrderUid, (StatusCode, String)> {
        tracing::info!("Creating order: {order:?}");
        let placement = self
            .http
            .post(format!("{API_HOST}{ORDERS_ENDPOINT}"))
            .json(order)
            .send()
            .await
            .unwrap();

        let status = placement.status();
        let body = placement.text().await.unwrap();

        match status {
            StatusCode::CREATED => Ok(serde_json::from_str(&body).unwrap()),
            code => Err((code, body)),
        }
    }

    /// Submit an [`model::quote::OrderQuote`].
    /// If the response status is not `200`, return the status and the body.
    pub async fn submit_quote(
        &self,
        quote: &OrderQuoteRequest,
    ) -> Result<OrderQuoteResponse, (StatusCode, String)> {
        let quoting = self
            .http
            .post(format!("{API_HOST}{QUOTING_ENDPOINT}"))
            .json(&quote)
            .send()
            .await
            .unwrap();

        let status = quoting.status();
        let body = quoting.text().await.unwrap();

        match status {
            StatusCode::OK => Ok(serde_json::from_str(&body).unwrap()),
            code => Err((code, body)),
        }
    }

    pub async fn get_native_price(
        &self,
        token: &Address,
    ) -> Result<NativeTokenPrice, (StatusCode, String)> {
        let response = self
            .http
            .get(format!("{API_HOST}/api/v1/token/{token:?}/native_price"))
            .send()
            .await
            .unwrap();

        let status = response.status();
        let body = response.text().await.unwrap();

        match status {
            StatusCode::OK => Ok(serde_json::from_str(&body).unwrap()),
            code => Err((code, body)),
        }
    }

    /// Retrieve an [`Order`]. If the respons status is not `200`, return the
    /// status and the body.
    pub async fn get_order(&self, uid: &OrderUid) -> Result<Order, (StatusCode, String)> {
        let response = self
            .http
            .get(format!("{API_HOST}{ORDERS_ENDPOINT}/{uid}"))
            .send()
            .await
            .unwrap();

        let status = response.status();
        let body = response.text().await.unwrap();

        match status {
            StatusCode::OK => Ok(serde_json::from_str(&body).unwrap()),
            code => Err((code, body)),
        }
    }

    pub async fn get_order_status(
        &self,
        uid: &OrderUid,
    ) -> Result<orderbook::dto::order::Status, (StatusCode, String)> {
        let response = self
            .http
            .get(format!("{API_HOST}{}", order_status_endpoint(uid)))
            .send()
            .await
            .unwrap();

        let status = response.status();
        let body = response.text().await.unwrap();

        match status {
            StatusCode::OK => {
                Ok(serde_json::from_str::<orderbook::dto::order::Status>(&body).unwrap())
            }
            code => Err((code, body)),
        }
    }

    pub async fn get_orders_for_tx(
        &self,
        tx_hash: &B256,
    ) -> Result<Vec<Order>, (StatusCode, String)> {
        let response = self
            .http
            .get(format!("{API_HOST}{}", orders_for_tx_endpoint(tx_hash)))
            .send()
            .await
            .unwrap();

        let status = response.status();
        let body = response.text().await.unwrap();

        match status {
            StatusCode::OK => Ok(serde_json::from_str(&body).unwrap()),
            code => Err((code, body)),
        }
    }

    pub async fn get_orders_for_owner(
        &self,
        owner: &Address,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<Order>, (StatusCode, String)> {
        let response = self
            .http
            .get(format!(
                "{API_HOST}{}",
                orders_for_owner(owner, offset, limit)
            ))
            .send()
            .await
            .unwrap();

        let status = response.status();
        let body = response.text().await.unwrap();

        match status {
            StatusCode::OK => Ok(serde_json::from_str(&body).unwrap()),
            code => Err((code, body)),
        }
    }

    pub async fn get_app_data_document(
        &self,
        app_data: AppDataHash,
    ) -> Result<AppDataDocument, (StatusCode, String)> {
        let response = self
            .http
            .get(format!("{API_HOST}/api/v1/app_data/{app_data:?}"))
            .send()
            .await
            .unwrap();

        let status = response.status();
        let body = response.text().await.unwrap();

        match status {
            StatusCode::OK => Ok(serde_json::from_str(&body).unwrap()),
            code => Err((code, body)),
        }
    }

    pub async fn get_app_data(
        &self,
        app_data: AppDataHash,
    ) -> Result<String, (StatusCode, String)> {
        Ok(self.get_app_data_document(app_data).await?.full_app_data)
    }

    pub async fn put_app_data_document(
        &self,
        app_data: Option<AppDataHash>,
        document: AppDataDocument,
    ) -> Result<String, (StatusCode, String)> {
        let url = match app_data {
            Some(app_data) => format!("{API_HOST}/api/v1/app_data/{app_data:?}"),
            None => format!("{API_HOST}/api/v1/app_data"),
        };
        let response = self.http.put(url).json(&document).send().await.unwrap();

        let status = response.status();
        let body = response.text().await.unwrap();

        if status.is_success() {
            let body = serde_json::from_str::<String>(&body).unwrap();
            Ok(body)
        } else {
            Err((status, body))
        }
    }

    pub async fn put_app_data(
        &self,
        app_data: Option<AppDataHash>,
        full_app_data: &str,
    ) -> Result<String, (StatusCode, String)> {
        self.put_app_data_document(
            app_data,
            AppDataDocument {
                full_app_data: full_app_data.to_owned(),
            },
        )
        .await
    }

    /// Get trades with pagination (v2 endpoint)
    pub async fn get_trades_v2(
        &self,
        order_uid: Option<&OrderUid>,
        owner: Option<&Address>,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<Trade>, (StatusCode, String)> {
        let mut query_params = vec![("offset", offset.to_string()), ("limit", limit.to_string())];
        if let Some(uid) = order_uid {
            query_params.push(("orderUid", uid.to_string()));
        }
        if let Some(owner_addr) = owner {
            query_params.push(("owner", owner_addr.to_string()));
        }

        let url = Url::from_str(format!("{API_HOST}/api/v2/trades").as_str())
            .expect("string should be a valid URL");

        let response = self
            .http
            .get(url)
            .query(&query_params)
            .send()
            .await
            .unwrap();

        let status = response.status();
        let body = response.text().await.unwrap();

        match status {
            StatusCode::OK => Ok(serde_json::from_str(&body).unwrap()),
            code => Err((code, body)),
        }
    }

    /// Get token metadata
    pub async fn get_token_metadata(
        &self,
        token: &Address,
    ) -> Result<orderbook::dto::TokenMetadata, (StatusCode, String)> {
        let response = self
            .http
            .get(format!("{API_HOST}/api/v1/token/{token:?}/metadata"))
            .send()
            .await
            .unwrap();

        let status = response.status();
        let body = response.text().await.unwrap();

        match status {
            StatusCode::OK => Ok(serde_json::from_str(&body).unwrap()),
            code => Err((code, body)),
        }
    }

    /// Get API version
    pub async fn get_api_version(&self) -> Result<String, (StatusCode, String)> {
        let response = self
            .http
            .get(format!("{API_HOST}{VERSION_ENDPOINT}"))
            .send()
            .await
            .unwrap();

        let status = response.status();
        let body = response.text().await.unwrap();

        match status {
            StatusCode::OK => Ok(body),
            code => Err((code, body)),
        }
    }

    /// Cancel a single order (deprecated endpoint)
    pub async fn cancel_order_single(
        &self,
        uid: &OrderUid,
        payload: &CancellationPayload,
    ) -> Result<String, (StatusCode, String)> {
        let response = self
            .http
            .delete(format!("{API_HOST}{ORDERS_ENDPOINT}/{uid}"))
            .json(payload)
            .send()
            .await
            .unwrap();

        let status = response.status();
        let body = response.text().await.unwrap();

        match status {
            StatusCode::OK => Ok(serde_json::from_str(&body).unwrap()),
            code => Err((code, body)),
        }
    }

    /// Get solver competition by auction ID (v1 - deprecated)
    pub async fn get_solver_competition_v1(
        &self,
        auction_id: AuctionId,
    ) -> Result<solver_competition::SolverCompetitionAPI, (StatusCode, String)> {
        let response = self
            .http
            .get(format!("{API_HOST}/api/v1/solver_competition/{auction_id}"))
            .send()
            .await
            .unwrap();

        let status = response.status();
        let body = response.text().await.unwrap();

        match status {
            StatusCode::OK => Ok(serde_json::from_str(&body).unwrap()),
            code => Err((code, body)),
        }
    }

    /// Get solver competition by transaction hash (v1 - deprecated)
    pub async fn get_solver_competition_by_tx_v1(
        &self,
        hash: B256,
    ) -> Result<solver_competition::SolverCompetitionAPI, (StatusCode, String)> {
        let response = self
            .http
            .get(format!(
                "{API_HOST}/api/v1/solver_competition/by_tx_hash/{hash:?}"
            ))
            .send()
            .await
            .unwrap();

        let status = response.status();
        let body = response.text().await.unwrap();

        match status {
            StatusCode::OK => Ok(serde_json::from_str(&body).unwrap()),
            code => Err((code, body)),
        }
    }

    /// Get latest solver competition (v1 - deprecated)
    pub async fn get_latest_solver_competition_v1(
        &self,
    ) -> Result<solver_competition::SolverCompetitionAPI, (StatusCode, String)> {
        let response = self
            .http
            .get(format!("{API_HOST}/api/v1/solver_competition/latest"))
            .send()
            .await
            .unwrap();

        let status = response.status();
        let body = response.text().await.unwrap();

        match status {
            StatusCode::OK => Ok(serde_json::from_str(&body).unwrap()),
            code => Err((code, body)),
        }
    }

    /// Get total surplus for a user (unstable endpoint)
    pub async fn get_user_total_surplus(
        &self,
        user: &Address,
    ) -> Result<U256, (StatusCode, String)> {
        let response = self
            .http
            .get(format!("{API_HOST}/api/v1/users/{user:?}/total_surplus"))
            .send()
            .await
            .unwrap();

        let status = response.status();
        let body = response.text().await.unwrap();

        match status {
            StatusCode::OK => {
                // Parse JSON response manually to extract totalSurplus
                let json: serde_json::Value = serde_json::from_str(&body).unwrap();
                let total_surplus_str = json["totalSurplus"].as_str().unwrap();
                Ok(U256::from_str_radix(total_surplus_str, 10).unwrap())
            }
            code => Err((code, body)),
        }
    }

    pub fn client(&self) -> &Client {
        &self.http
    }

    /// Returns the underlying postgres connection pool that can be used do
    /// execute raw SQL queries.
    pub fn db(&self) -> &Db {
        &self.db
    }

    async fn mint_block(&self) {
        tracing::info!("mining block");
        self.web3.provider.evm_mine(None).await.unwrap();
    }
}

pub async fn clear_database() {
    tracing::info!("Clearing database.");

    async fn truncate_tables() -> Result<(), sqlx::Error> {
        let mut db = sqlx::PgConnection::connect(LOCAL_DB_URL).await?;
        let mut db = db.begin().await?;
        database::clear_DANGER_(&mut db).await?;
        db.commit().await
    }

    // This operation can fail when postgres detects a deadlock.
    // It will terminate one of the deadlocking requests and if it decideds
    // to terminate this request we need to retry it.
    let mut attempt = 0;
    loop {
        match truncate_tables().await {
            Ok(_) => return,
            Err(err) => {
                tracing::error!(?err, "failed to truncate tables");
            }
        }
        attempt += 1;
        if attempt >= 10 {
            panic!("repeatedly failed to clear DB");
        }
    }
}

pub async fn ensure_e2e_readonly_user() {
    use sqlx::{Executor, Row};

    const PSQL_DUPLICATE_OBJECT_ERROR_CODE: &str = "42710";

    tracing::info!("Ensuring read-only user exists");
    let mut db = sqlx::PgConnection::connect(LOCAL_DB_URL)
        .await
        .expect("Database connection error");
    let mut db: sqlx::Transaction<'_, sqlx::Postgres> = db
        .begin()
        .await
        .expect("Database transaction creation error");
    let current_db: String = db
        .fetch_one("SELECT current_database();")
        .await
        .expect("Current database name fetching error")
        .get(0);

    let res = db
        .execute(
            format!(
                r#"
    CREATE ROLE readonly WITH LOGIN PASSWORD 'password';
    GRANT CONNECT ON DATABASE "{current_db}" TO readonly;
    GRANT USAGE ON SCHEMA public TO readonly;
    GRANT SELECT ON ALL TABLES IN SCHEMA public TO readonly;
    GRANT SELECT ON ALL SEQUENCES IN SCHEMA public TO readonly;
    ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT ON TABLES TO readonly;
    ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT ON SEQUENCES TO readonly;
    "#
            )
            .as_str(),
        )
        .await;

    match res {
        Err(sqlx::Error::Database(e))
            if e.code()
                .is_some_and(|c| c == PSQL_DUPLICATE_OBJECT_ERROR_CODE) =>
        {
            // this is considered expected, if multiple tests are run against the same
            // database
            tracing::info!("Read-only user already exists! {:?}", e);
        }
        Err(e) => {
            tracing::error!("Read-only user creation failed {:?}", e);
            panic!("Read-only user creation failed {:?}", e);
        }
        Ok(_) => {
            tracing::info!("Read only user created");
            db.commit().await.expect("Transaction commit error");
        }
    }
}

pub type Db = sqlx::Pool<sqlx::Postgres>;

/// Clap does not allow you to overwrite CLI arguments easily. This
/// function loops over all provided arguments and only keeps the
/// last one if there are multiples.
fn ignore_overwritten_cli_params(mut params: Vec<String>) -> Vec<String> {
    let mut defined_args = HashMap::new();
    params.reverse(); // reverse to give later params higher priority
    params.retain(move |param| {
        let Some((arg, value)) = param.split_once('=') else {
            return true; // keep anything we can't parse (e.g. program name)
        };
        match defined_args.entry(arg.to_string()) {
            Entry::Occupied(val) => {
                tracing::info!(
                    ignored = ?param,
                    kept = format!("{arg}={}", val.get()),
                    "ignoring overwritten CLI argument"
                );
                false
            }
            Entry::Vacant(slot) => {
                slot.insert(value.to_string());
                true
            }
        }
    });
    params.reverse(); // reverse to restore original order again
    params
}
