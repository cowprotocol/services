use {
    driver::infra::notify::liquidity_sources::liquorice::client::request::v1::intent_origin::notification,
    serde_json::json,
    std::{convert::Infallible, net::SocketAddr, sync::Arc},
    tokio::sync::{Mutex, MutexGuard},
    warp::{Filter, Rejection},
};

pub struct State {
    pub notification_requests: Vec<notification::post::Request>,
}

pub struct LiquoriceApi {
    state: Arc<Mutex<State>>,
    pub port: u16,
}

impl LiquoriceApi {
    /// Creates new mocked `LiquoriceApi` with internal state
    pub async fn start() -> Self {
        let state = Arc::new(Mutex::new(State {
            notification_requests: Default::default(),
        }));

        let addr: SocketAddr = ([0, 0, 0, 0], 0).into();
        let server = warp::serve(Self::notification_route(state.clone()));
        let (addr, server) = server.bind_ephemeral(addr);
        let port = addr.port();
        assert!(port > 0, "assigned port must be greater than 0");

        tokio::spawn(async move {
            server.await;
        });

        tracing::info!("Started Liquorice API server at {}", addr);

        Self { state, port }
    }

    pub fn notification_route(
        state: Arc<Mutex<State>>,
    ) -> impl Filter<Extract = (warp::reply::Json,), Error = Rejection> + Clone {
        warp::path!("v1" / "intent-origin" / "notification")
            .and(warp::post())
            .and(warp::body::json::<notification::post::Request>())
            .and_then(move |request| {
                let state = state.clone();
                async move {
                    state.lock().await.notification_requests.push(request);
                    Ok::<warp::reply::Json, Infallible>(warp::reply::json(&json!({})))
                }
            })
    }

    pub async fn get_state(&self) -> MutexGuard<'_, State> {
        self.state.lock().await
    }
}
