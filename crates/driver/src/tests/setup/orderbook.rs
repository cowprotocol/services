use {
    axum::{extract::Path, http::StatusCode, routing::get, Router},
    std::net::SocketAddr,
};

/// A mocked orderbook service that provides `/v1/app_data/{app_data_hash}` API.
/// Always returns 404 Not Found.
pub struct Orderbook {
    pub addr: SocketAddr,
}

impl Orderbook {
    pub fn start() -> Self {
        let app = Router::new().route("/v1/app_data/:app_data", get(Self::mock_handler));
        let server =
            axum::Server::bind(&"0.0.0.0:0".parse().unwrap()).serve(app.into_make_service());
        let addr = server.local_addr();
        println!("Orderbook mock server listening on {}", addr);

        tokio::spawn(server);

        Orderbook { addr }
    }

    /// Default mock handler that always returns 404 Not Found.
    async fn mock_handler(Path(app_data): Path<String>) -> StatusCode {
        println!("Orderbook received an app_data request: {}", app_data);
        StatusCode::NOT_FOUND
    }
}
