use {
    crate::{domain::competition::order::app_data::AppData, tests::setup::Order},
    axum::{
        extract::Path,
        http::StatusCode,
        response::IntoResponse,
        routing::get,
        Extension,
        Json,
        Router,
    },
    std::{collections::HashMap, net::SocketAddr},
};

/// A mocked orderbook service that provides `/v1/app_data/{app_data_hash}` API.
/// Uses in-memory app_data storage implemented with an axum extension layer and
/// a `HashMap` which is sufficient due to read-only concurrent access.
pub struct Orderbook {
    pub addr: SocketAddr,
}

impl Orderbook {
    /// Starts the orderbook server.
    /// Uses the provided orders to populate the app_data storage where only
    /// full app_data is stored. The server listens on a random port.
    ///
    /// # Returns
    /// The `Orderbook` instance with the server listening address.
    pub fn start(orders: &[Order]) -> Self {
        let app_data_storage = orders
            .iter()
            .filter_map(|order| {
                if let AppData::Full(validated_data) = &order.app_data {
                    Some((
                        app_data::AppDataHash(order.app_data.hash().0 .0),
                        app_data::Root::new(Some(validated_data.protocol.clone())),
                    ))
                } else {
                    None
                }
            })
            .collect::<HashMap<_, _>>();

        let app = Router::new()
            .route("/v1/app_data/:app_data", get(Self::app_data_handler))
            .layer(Extension(app_data_storage));
        let server =
            axum::Server::bind(&"0.0.0.0:0".parse().unwrap()).serve(app.into_make_service());
        let addr = server.local_addr();

        tracing::info!("Orderbook mock server listening on {}", addr);

        tokio::spawn(server);

        Orderbook { addr }
    }

    async fn app_data_handler(
        Path(app_data): Path<String>,
        Extension(app_data_storage): Extension<HashMap<app_data::AppDataHash, app_data::Root>>,
    ) -> impl IntoResponse {
        tracing::debug!("Orderbook received an app_data request: {}", app_data);

        let app_data_hash = match app_data.parse::<app_data::AppDataHash>() {
            Ok(hash) => hash,
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "error": "Invalid app_data format" })),
                )
                    .into_response();
            }
        };

        if let Some(full_app_data) = app_data_storage.get(&app_data_hash) {
            return Json(full_app_data.clone()).into_response();
        }

        StatusCode::NOT_FOUND.into_response()
    }
}
