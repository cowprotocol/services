use {
    super::{colocation::start_legacy_solver, TestAccount},
    crate::setup::{
        colocation::{self, SolverEngine},
        wait_for_condition,
        Contracts,
        TIMEOUT,
    },
    app_data::{AppDataDocument, AppDataHash},
    autopilot::infra::persistence::dto,
    clap::Parser,
    ethcontract::{H256, U256},
    model::{
        order::{Order, OrderCreation, OrderUid},
        quote::{OrderQuoteRequest, OrderQuoteResponse},
        solver_competition::SolverCompetitionAPI,
        trade::Trade,
    },
    reqwest::{Client, StatusCode, Url},
    sqlx::Connection,
    std::time::Duration,
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

    pub async fn build(self, contracts: &Contracts) -> Services {
        Services {
            contracts,
            http: Client::builder().timeout(self.timeout).build().unwrap(),
            db: sqlx::PgPool::connect(LOCAL_DB_URL).await.unwrap(),
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
}

impl<'a> Services<'a> {
    pub async fn new(contracts: &'a Contracts) -> Services<'a> {
        Self {
            contracts,
            http: Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap(),
            db: sqlx::PgPool::connect(LOCAL_DB_URL).await.unwrap(),
        }
    }

    pub fn builder() -> ServicesBuilder {
        ServicesBuilder::new()
    }

    fn api_autopilot_arguments() -> impl Iterator<Item = String> {
        [
            "--price-estimators=None".to_string(),
            "--native-price-estimators=test_quoter".to_string(),
            "--amount-to-estimate-prices-with=1000000000000000000".to_string(),
            "--block-stream-poll-interval=1s".to_string(),
            "--trade-simulator=Web3".to_string(),
            "--simulation-node-url=http://localhost:8545".to_string(),
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
    pub fn start_autopilot(&self, solve_deadline: Option<Duration>, extra_args: Vec<String>) {
        let solve_deadline = solve_deadline.unwrap_or(Duration::from_secs(2));

        let args = [
            "autopilot".to_string(),
            "--auction-update-interval=1s".to_string(),
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
        let solver_endpoint =
            colocation::start_baseline_solver(self.contracts.weth.address()).await;
        colocation::start_driver(
            self.contracts,
            vec![SolverEngine {
                name: "test_solver".into(),
                account: solver,
                endpoint: solver_endpoint,
            }],
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
        );
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

    /// Starts a basic version of the protocol with a single legacy solver and
    /// quoter.
    pub async fn start_protocol_legacy_solver(
        &self,
        solver: TestAccount,
        solver_endpoint: Option<Url>,
        quoter_endpoint: Option<Url>,
        chain_id: Option<U256>,
    ) {
        let external_solver_endpoint =
            solver_endpoint.unwrap_or("http://localhost:8000/solve".parse().unwrap());
        let colocated_solver_endpoint =
            start_legacy_solver(external_solver_endpoint, chain_id).await;

        let external_quoter_endpoint =
            quoter_endpoint.unwrap_or("http://localhost:8000/quote".parse().unwrap());
        let colocated_quoter_endpoint =
            start_legacy_solver(external_quoter_endpoint, chain_id).await;

        colocation::start_driver(
            self.contracts,
            vec![
                SolverEngine {
                    name: "test_solver".into(),
                    account: solver.clone(),
                    endpoint: colocated_solver_endpoint,
                },
                SolverEngine {
                    name: "test_quoter".into(),
                    account: solver,
                    endpoint: colocated_quoter_endpoint,
                },
            ],
        );
        self.start_autopilot(
            Some(Duration::from_secs(11)),
            vec![
                "--drivers=test_solver|http://localhost:11088/test_solver".to_string(),
                "--price-estimation-drivers=test_quoter|http://localhost:11088/test_quoter"
                    .to_string(),
            ],
        );
        self.start_api(vec![
            "--price-estimation-drivers=test_quoter|http://localhost:11088/test_quoter".to_string(),
        ])
        .await;
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

    pub async fn get_auction(&self) -> dto::AuctionWithId {
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
            .post(&format!("{API_HOST}{QUOTING_ENDPOINT}"))
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

    pub async fn solvable_orders(&self) -> usize {
        self.get_auction().await.auction.orders.len()
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
}

pub async fn clear_database() {
    tracing::info!("Clearing database.");
    let mut db = sqlx::PgConnection::connect(LOCAL_DB_URL).await.unwrap();
    let mut db = db.begin().await.unwrap();
    database::clear_DANGER_(&mut db).await.unwrap();
    db.commit().await.unwrap();
}

pub type Db = sqlx::Pool<sqlx::Postgres>;
