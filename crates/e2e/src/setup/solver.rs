//! Mock solver for testing purposes. It returns a custom solution.

use {
    app_data::AppDataHash,
    axum::Json,
    ethcontract::common::abi::ethereum_types::Address,
    model::{
        order::{BuyTokenDestination, OrderData, OrderKind, SellTokenSource},
        signature::EcdsaSigningScheme,
        DomainSeparator,
    },
    reqwest::Url,
    solvers_dto::{
        auction::Auction,
        solution::{Asset, Kind, Solution, Solutions},
    },
    std::sync::{Arc, Mutex},
    tokio::signal::{unix, unix::SignalKind},
    tracing::Instrument,
    warp::hyper,
    web3::signing::SecretKeyRef,
};

/// A solver that does not implement any solving logic itself and instead simply
/// forwards a single hardcoded solution.
pub struct Mock {
    /// The currently configured solution to return.
    solution: Arc<Mutex<Option<Solution>>>,
    /// Under which URL the solver is reachable by a driver.
    pub url: Url,
}

impl Mock {
    /// Instructs the solver to return a new solution from now on.
    pub fn configure_solution(&self, solution: Option<Solution>) {
        *self.solution.lock().unwrap() = solution;
    }
}

impl Default for Mock {
    fn default() -> Self {
        let solution = Arc::new(Mutex::new(None));

        let app = axum::Router::new()
            .layer(tower::ServiceBuilder::new().layer(
                tower_http::limit::RequestBodyLimitLayer::new(REQUEST_BODY_LIMIT),
            ))
            .route("/solve", axum::routing::post(solve))
            .layer(
                tower::ServiceBuilder::new().layer(tower_http::trace::TraceLayer::new_for_http()),
            )
            .with_state(solution.clone())
            // axum's default body limit needs to be disabled to not have the default limit on top of our custom limit
            .layer(axum::extract::DefaultBodyLimit::disable());

        let make_svc = observe::make_service_with_task_local_storage!(app);

        let server = axum::Server::bind(&"0.0.0.0:0".parse().unwrap()).serve(make_svc);

        let mock = Mock {
            solution,
            url: format!("http://{}", server.local_addr()).parse().unwrap(),
        };

        tokio::task::spawn(server.with_graceful_shutdown(shutdown_signal()));

        mock
    }
}

async fn solve(
    state: axum::extract::State<Arc<Mutex<Option<Solution>>>>,
    Json(auction): Json<Auction>,
) -> (axum::http::StatusCode, Json<Solutions>) {
    let auction_id = auction.id.unwrap_or_default();
    let solutions = state.lock().unwrap().iter().cloned().collect();
    let solutions = Solutions { solutions };
    tracing::trace!(?auction_id, ?solutions, "/solve");
    (axum::http::StatusCode::OK, Json(solutions))
}

const REQUEST_BODY_LIMIT: usize = 10 * 1024 * 1024;

#[cfg(unix)]
async fn shutdown_signal() {
    // Intercept main signals for graceful shutdown.
    // Kubernetes sends sigterm, whereas locally sigint (ctrl-c) is most common.
    let mut interrupt = unix::signal(SignalKind::interrupt()).unwrap();
    let mut terminate = unix::signal(SignalKind::terminate()).unwrap();
    tokio::select! {
        _ = interrupt.recv() => (),
        _ = terminate.recv() => (),
    };
}

#[derive(Clone, Debug)]
pub struct JitOrder {
    pub owner: Address,
    pub sell: Asset,
    pub buy: Asset,
    pub kind: OrderKind,
    pub partially_fillable: bool,
    pub valid_to: u32,
    pub app_data: AppDataHash,
    pub receiver: Address,
}

impl JitOrder {
    fn data(&self) -> OrderData {
        OrderData {
            sell_token: self.sell.token,
            buy_token: self.buy.token,
            receiver: self.receiver.into(),
            sell_amount: self.sell.amount,
            buy_amount: self.buy.amount,
            valid_to: self.valid_to,
            app_data: AppDataHash(self.app_data.0),
            fee_amount: 0.into(),
            kind: self.kind,
            partially_fillable: self.partially_fillable,
            sell_token_balance: Default::default(),
            buy_token_balance: Default::default(),
        }
    }

    pub fn sign(
        self,
        signing_scheme: EcdsaSigningScheme,
        domain: &DomainSeparator,
        key: SecretKeyRef,
    ) -> solvers_dto::solution::JitOrder {
        let data = self.data();
        let signature = match model::signature::EcdsaSignature::sign(
            signing_scheme,
            domain,
            &data.hash_struct(),
            key,
        )
        .to_signature(signing_scheme)
        {
            model::signature::Signature::Eip712(signature) => signature.to_bytes().to_vec(),
            model::signature::Signature::EthSign(signature) => signature.to_bytes().to_vec(),
            model::signature::Signature::Eip1271(signature) => signature,
            model::signature::Signature::PreSign => panic!("Not supported PreSigned JIT orders"),
        };
        solvers_dto::solution::JitOrder {
            sell_token: data.sell_token,
            buy_token: data.buy_token,
            receiver: data.receiver.unwrap_or_default(),
            sell_amount: data.sell_amount,
            buy_amount: data.buy_amount,
            valid_to: data.valid_to,
            app_data: data.app_data.0,
            fee_amount: data.fee_amount,
            kind: match data.kind {
                OrderKind::Buy => Kind::Buy,
                OrderKind::Sell => Kind::Sell,
            },
            partially_fillable: data.partially_fillable,
            sell_token_balance: match data.sell_token_balance {
                SellTokenSource::Erc20 => solvers_dto::solution::SellTokenBalance::Erc20,
                SellTokenSource::External => solvers_dto::solution::SellTokenBalance::External,
                SellTokenSource::Internal => solvers_dto::solution::SellTokenBalance::Internal,
            },
            buy_token_balance: match data.buy_token_balance {
                BuyTokenDestination::Erc20 => solvers_dto::solution::BuyTokenBalance::Erc20,
                BuyTokenDestination::Internal => solvers_dto::solution::BuyTokenBalance::Internal,
            },
            signing_scheme: match signing_scheme {
                EcdsaSigningScheme::Eip712 => solvers_dto::solution::SigningScheme::Eip712,
                EcdsaSigningScheme::EthSign => solvers_dto::solution::SigningScheme::EthSign,
            },
            signature,
        }
    }
}
