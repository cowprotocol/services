use {
    super::TestAccount,
    crate::setup::{
        colocation::{self, SolverEngine},
        wait_for_condition,
        Contracts,
        OnchainComponents,
        TIMEOUT,
    },
    app_data::{AppDataDocument, AppDataHash},
    autopilot::infra::persistence::dto,
    clap::Parser,
    ethcontract::{H160, H256},
    model::{
        order::{Order, OrderCreation, OrderUid},
        quote::{OrderQuoteRequest, OrderQuoteResponse},
        solver_competition::SolverCompetitionAPI,
        trade::Trade,
    },
    reqwest::{Client, StatusCode, Url},
    shared::ethrpc::Web3,
    sqlx::Connection,
    std::{ops::DerefMut, time::Duration},
    web3::Transport,
};

pub const API_HOST: &str = "http://127.0.0.1:8080";
pub const ORDERS_ENDPOINT: &str = "/api/v1/orders";
pub const QUOTING_ENDPOINT: &str = "/api/v1/quote";
pub const ACCOUNT_ENDPOINT: &str = "/api/v1/account";
pub const AUCTION_ENDPOINT: &str = "/api/v1/auction";
pub const TRADES_ENDPOINT: &str = "/api/v1/trades";
pub const VERSION_ENDPOINT: &str = "/api/v1/version";
pub const SOLVER_COMPETITION_ENDPOINT: &str = "/api/v1/solver_competition";
const LOCAL_DB_URL: &str = "postgresql://";

fn order_status_endpoint(uid: &OrderUid) -> String {
    format!("/api/v1/orders/{uid}/status")
}

fn orders_for_tx_endpoint(tx_hash: &H256) -> String {
    format!("/api/v1/transactions/{tx_hash:?}/orders")
}

fn orders_for_owner(owner: &H160, offset: u64, limit: u64) -> String {
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

    pub async fn build(self, onchain_components: &OnchainComponents) -> Services {
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

    fn api_autopilot_arguments() -> impl Iterator<Item = String> {
        [
            "--native-price-estimators=test_quoter|http://localhost:11088/test_solver".to_string(),
            "--amount-to-estimate-prices-with=1000000000000000000".to_string(),
            "--block-stream-poll-interval=1s".to_string(),
            "--simulation-node-url=http://localhost:8545".to_string(),
            "--native-price-cache-max-age=2s".to_string(),
            "--native-price-prefetch-time=500ms".to_string(),
        ]
        .into_iter()
    }

    fn api_autopilot_solver_arguments(&self) -> impl Iterator<Item = String> {
        [
            "--baseline-sources=None".to_string(),
            "--network-block-interval=1s".to_string(),
            "--solver-competition-auth=super_secret_key".to_string(),
            format!(
                "--settlement-contract-address={:?}",
                self.contracts.gp_settlement.address()
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
    /// deadline in case the solution would start to revert at some point)
    pub async fn start_autopilot(&self, solve_deadline: Option<Duration>, extra_args: Vec<String>) {
        let solve_deadline = solve_deadline.unwrap_or(Duration::from_secs(2));

        let args = [
            "autopilot".to_string(),
            "--max-run-loop-delay=100ms".to_string(),
            "--run-loop-native-price-timeout=500ms".to_string(),
            format!("--ethflow-contract={:?}", self.contracts.ethflow.address()),
            "--skip-event-sync=true".to_string(),
            format!("--solve-deadline={solve_deadline:?}"),
        ]
        .into_iter()
        .chain(self.api_autopilot_solver_arguments())
        .chain(Self::api_autopilot_arguments())
        .chain(extra_args);

        let args = autopilot::arguments::Arguments::try_parse_from(args).unwrap();
        tokio::task::spawn(autopilot::run(args));
        self.wait_until_autopilot_ready().await;
    }

    /// Start the api service in a background tasks.
    /// Wait until the service is responsive.
    pub async fn start_api(&self, extra_args: Vec<String>) {
        let args = [
            "orderbook".to_string(),
            format!(
                "--hooks-contract-address={:?}",
                self.contracts.hooks.address()
            ),
            "--quote-timeout=10s".to_string(),
            "--quote-verification=enforce-when-possible".to_string(),
        ]
        .into_iter()
        .chain(self.api_autopilot_solver_arguments())
        .chain(Self::api_autopilot_arguments())
        .chain(extra_args);

        let args = orderbook::arguments::Arguments::try_parse_from(args).unwrap();
        tokio::task::spawn(orderbook::run(args));

        Self::wait_for_api_to_come_up().await;
    }

    /// Starts a basic version of the protocol with a single baseline solver.
    pub async fn start_protocol(&self, solver: TestAccount) {
        self.start_protocol_with_args(Default::default(), solver)
            .await;
    }

    pub async fn start_protocol_with_args(&self, args: ExtraServiceArgs, solver: TestAccount) {
        colocation::start_driver(
            self.contracts,
            vec![
                colocation::start_baseline_solver(
                    "test_solver".into(),
                    solver,
                    self.contracts.weth.address(),
                    vec![],
                    1,
                    true,
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
                    "--drivers=test_solver|http://localhost:11088/test_solver".to_string(),
                    "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver"
                        .to_string(),
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
        }];

        let (autopilot_args, api_args) = if run_baseline {
            solvers.push(
                colocation::start_baseline_solver(
                    "baseline_solver".into(),
                    solver,
                    self.contracts.weth.address(),
                    vec![],
                    1,
                    true,
                )
                .await,
            );

            // Here we call the baseline_solver "test_quoter" to make the native price
            // estimation use the baseline_solver instead of the test_quoter
            let autopilot_args = vec![
                "--drivers=test_solver|http://localhost:11088/test_solver".to_string(),
                "--price-estimation-drivers=test_quoter|http://localhost:11088/baseline_solver,test_solver|http://localhost:11088/test_solver".to_string(),
                "--native-price-estimators=test_quoter|http://localhost:11088/baseline_solver,test_solver|http://localhost:11088/test_solver".to_string(),
            ];
            let api_args = vec![
                "--price-estimation-drivers=test_quoter|http://localhost:11088/baseline_solver,test_solver|http://localhost:11088/test_solver".to_string(),
                "--native-price-estimators=test_quoter|http://localhost:11088/baseline_solver,test_solver|http://localhost:11088/test_solver".to_string(),
            ];
            (autopilot_args, api_args)
        } else {
            let autopilot_args = vec![
                "--drivers=test_solver|http://localhost:11088/test_solver".to_string(),
                "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver"
                    .to_string(),
                "--native-price-estimators=test_quoter|http://localhost:11088/test_solver"
                    .to_string(),
            ];

            let api_args = vec![
                "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver"
                    .to_string(),
                "--native-price-estimators=test_quoter|http://localhost:11088/test_solver"
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
        hash: H256,
    ) -> Result<SolverCompetitionAPI, StatusCode> {
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

    pub async fn get_latest_solver_competition(&self) -> Result<SolverCompetitionAPI, StatusCode> {
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
        tx_hash: &H256,
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
        owner: &H160,
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
        self.web3
            .transport()
            .execute("evm_mine", vec![])
            .await
            .unwrap();
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

pub type Db = sqlx::Pool<sqlx::Postgres>;
