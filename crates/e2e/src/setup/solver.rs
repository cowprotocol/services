//! Mock solver for testing purposes. It returns a custom solution.

use {
    app_data::AppDataHash,
    axum::Json,
    ethcontract::{common::abi::ethereum_types::Address, jsonrpc::serde::Serialize},
    model::{
        order::{BuyTokenDestination, OrderData, OrderKind, SellTokenSource},
        signature::EcdsaSigningScheme,
        DomainSeparator,
    },
    solvers_dto::{
        auction::Auction,
        solution::{Asset, Kind, Solution, Solutions, Trade},
    },
    std::{future::Future, net::SocketAddr, sync::Arc},
    tokio::{
        signal::{unix, unix::SignalKind},
        sync::oneshot,
    },
    tracing::Instrument,
    warp::hyper,
    web3::signing::SecretKeyRef,
};

pub struct Mock {
    solution: Solution,
}

impl Mock {
    pub fn new(solution: Solution) -> Self {
        Self { solution }
    }

    /// Returns the specified solution with the same order UID as the order UID
    /// in the auction.
    pub async fn solve(&self, auction: Auction) -> Solutions {
        let mut solution = self.solution.clone();
        // Return the same order UID as the order UID in the auction
        solution
            .trades
            .iter_mut()
            .filter_map(|trade| match trade {
                Trade::Fulfillment(fulfillment) => Some(fulfillment),
                Trade::Jit(_) => None,
            })
            .zip(auction.orders.iter())
            .for_each(|(fulfillment, order)| fulfillment.order = order.uid);

        Solutions {
            solutions: vec![solution],
        }
    }
}

#[derive(Debug)]
pub struct Config {
    pub log: String,
    pub addr: SocketAddr,
    pub solution: Solution,
}

pub async fn run_mock(config: Config, bind: Option<oneshot::Sender<SocketAddr>>) {
    observe::tracing::initialize_reentrant(&config.log);
    tracing::info!("running mock solver engine with {config:#?}");

    let solver = Mock::new(config.solution);

    Api {
        addr: config.addr,
        solver,
    }
    .serve(bind, shutdown_signal())
    .await
    .unwrap();
}

pub async fn solve(
    state: axum::extract::State<Arc<Mock>>,
    Json(auction): Json<Auction>,
) -> (axum::http::StatusCode, Json<Response<Solutions>>) {
    let handle_request = async {
        let auction_id = auction.id.unwrap_or_default();
        let solutions = state
            .solve(auction)
            .instrument(tracing::info_span!("auction", id = %auction_id))
            .await;

        tracing::trace!(?auction_id, ?solutions);

        (axum::http::StatusCode::OK, Json(Response::Ok(solutions)))
    };

    handle_request
        .instrument(tracing::info_span!("/solve"))
        .await
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Response<T> {
    Ok(T),
    Err(Error),
}

#[derive(Debug, Serialize)]
pub struct Error {
    pub message: &'static str,
}

impl From<&'static str> for Error {
    fn from(message: &'static str) -> Self {
        Self { message }
    }
}

const REQUEST_BODY_LIMIT: usize = 10 * 1024 * 1024;

pub struct Api {
    pub addr: SocketAddr,
    pub solver: Mock,
}

impl Api {
    pub async fn serve(
        self,
        bind: Option<oneshot::Sender<SocketAddr>>,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> Result<(), hyper::Error> {
        let app = axum::Router::new()
            .layer(tower::ServiceBuilder::new().layer(
                tower_http::limit::RequestBodyLimitLayer::new(REQUEST_BODY_LIMIT),
            ))
            .route("/solve", axum::routing::post(solve))
            .layer(
                tower::ServiceBuilder::new().layer(tower_http::trace::TraceLayer::new_for_http()),
            )
            .with_state(Arc::new(self.solver))
            // axum's default body limit needs to be disabled to not have the default limit on top of our custom limit
            .layer(axum::extract::DefaultBodyLimit::disable());

        let make_svc = observe::make_service_with_task_local_storage!(app);

        let server = axum::Server::bind(&self.addr).serve(make_svc);
        if let Some(bind) = bind {
            let _ = bind.send(server.local_addr());
        }

        server.with_graceful_shutdown(shutdown).await
    }
}

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
