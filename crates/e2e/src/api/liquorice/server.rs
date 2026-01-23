use {
    axum::Json,
    driver::infra::notify::liquidity_sources::liquorice::client::request::v1::intent_origin::notification,
    serde_json::json,
    std::{
        net::{Ipv4Addr, SocketAddr},
        sync::Arc,
    },
    tokio::sync::{Mutex, MutexGuard},
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

        let app = axum::Router::new()
            .route(
                "/v1/intent-origin/notification",
                axum::routing::post(notification_handler),
            )
            .with_state(state.clone());

        let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0));
        let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let port = addr.port();
        assert!(port > 0, "assigned port must be greater than 0");

        tokio::spawn(async move {
            if let Err(err) = axum::serve(listener, app).await {
                tracing::error!(?err, "Liquorice API server failed");
                panic!("Liquorice test server crashed: {}", err);
            }
        });

        tracing::info!("Started Liquorice API server at {}", addr);

        Self { state, port }
    }

    pub async fn get_state(&self) -> MutexGuard<'_, State> {
        self.state.lock().await
    }
}

async fn notification_handler(
    axum::extract::State(state): axum::extract::State<Arc<Mutex<State>>>,
    Json(request): Json<notification::post::Request>,
) -> Json<serde_json::Value> {
    state.lock().await.notification_requests.push(request);
    Json(json!({}))
}
