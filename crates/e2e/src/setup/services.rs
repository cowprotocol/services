use {
    crate::{
        nodes::NODE_HOST,
        setup::{wait_for_condition, Contracts, TIMEOUT},
    },
    clap::Parser,
    ethcontract::{H160, H256},
    model::{
        app_data::{AppDataDocument, AppDataHash},
        auction::AuctionWithId,
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

    fn api_autopilot_arguments() -> impl Iterator<Item = String> {
        [
            "--price-estimators=Baseline|0x0000000000000000000000000000000000000001".to_string(),
            "--native-price-estimators=Baseline".to_string(),
            "--amount-to-estimate-prices-with=1000000000000000000".to_string(),
            "--block-stream-poll-interval-seconds=1".to_string(),
        ]
        .into_iter()
    }

    fn api_autopilot_solver_arguments(&self) -> impl Iterator<Item = String> {
        [
            "--baseline-sources=None".to_string(),
            "--network-block-interval=1".to_string(),
            "--solver-competition-auth=super_secret_key".to_string(),
            format!(
                "--custom-univ2-baseline-sources={:?}|{:?}",
                self.contracts.uniswap_v2_router.address(),
                H256(shared::sources::uniswap_v2::UNISWAP_INIT),
            ),
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
    pub fn start_autopilot(&self, extra_args: Vec<String>) {
        let args = [
            "autopilot".to_string(),
            "--auction-update-interval=1".to_string(),
            format!("--ethflow-contract={:?}", self.contracts.ethflow.address()),
            "--skip-event-sync=true".to_string(),
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
            "--enable-presign-orders=true".to_string(),
            "--enable-eip1271-orders=true".to_string(),
            format!(
                "--hooks-contract-address={:?}",
                self.contracts.hooks.address()
            ),
        ]
        .into_iter()
        .chain(self.api_autopilot_solver_arguments())
        .chain(Self::api_autopilot_arguments())
        .chain(extra_args.into_iter());

        let args = orderbook::arguments::Arguments::try_parse_from(args).unwrap();
        tokio::task::spawn(orderbook::run(args));

        Self::wait_for_api_to_come_up().await;
    }

    /// Start the solver service in a background task.
    pub fn start_old_driver(&self, private_key: &[u8; 32], extra_args: Vec<String>) {
        let args = [
            "solver".to_string(),
            format!("--solver-account={}", hex::encode(private_key)),
            "--settle-interval=1".to_string(),
            format!("--transaction-submission-nodes={NODE_HOST}"),
            format!("--ethflow-contract={:?}", self.contracts.ethflow.address()),
        ]
        .into_iter()
        .chain(self.api_autopilot_solver_arguments())
        .chain(extra_args);

        let args = solver::arguments::Arguments::try_parse_from(args).unwrap();
        tokio::task::spawn(solver::run::run(args));
    }

    /// Start the solver service in a background task with a custom http solver
    /// only.
    pub fn start_old_driver_custom_solver(
        &self,
        solver_url: Option<Url>,
        solver_account: H160,
        extra_args: Vec<String>,
    ) {
        let args = [
            "solver".to_string(),
            format!(
                "--external-solvers=Custom|{}|{:#x}|false",
                solver_url
                    .unwrap_or("http://localhost:8000".parse().unwrap())
                    .as_str(),
                solver_account
            ),
            "--solvers=None".to_string(),
            format!("--solver-account={:#x}", solver_account),
            "--settle-interval=1".to_string(),
            format!("--transaction-submission-nodes={NODE_HOST}"),
            format!("--ethflow-contract={:?}", self.contracts.ethflow.address()),
        ]
        .into_iter()
        .chain(self.api_autopilot_solver_arguments())
        .chain(extra_args);

        let args = solver::arguments::Arguments::try_parse_from(args).unwrap();
        tokio::task::spawn(solver::run(args));
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

    pub async fn get_auction(&self) -> AuctionWithId {
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
        // print api host and orders endpoint
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
        app_data: AppDataHash,
        document: AppDataDocument,
    ) -> Result<(), (StatusCode, String)> {
        let response = self
            .http
            .put(format!("{API_HOST}/api/v1/app_data/{app_data:?}"))
            .json(&document)
            .send()
            .await
            .unwrap();

        let status = response.status();
        let body = response.text().await.unwrap();

        if status.is_success() {
            Ok(())
        } else {
            Err((status, body))
        }
    }

    pub async fn put_app_data(
        &self,
        app_data: AppDataHash,
        full_app_data: &str,
    ) -> Result<(), (StatusCode, String)> {
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
