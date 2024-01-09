use {
    shared::zeroex_api::{OrderRecord, OrdersQuery, ZeroExResponseError},
    std::{collections::HashMap, net::SocketAddr, str::FromStr, sync::Arc},
    warp::{Filter, Reply},
    web3::types::H160,
};

type OrdersHandler =
    Arc<dyn Fn(&OrdersQuery) -> Result<Vec<OrderRecord>, ZeroExResponseError> + Send + Sync>;

#[derive(Default)]
pub struct ZeroExApiBuilder {
    orders_handler: Option<OrdersHandler>,
}

impl ZeroExApiBuilder {
    pub fn with_orders_handler(mut self, handler: OrdersHandler) -> Self {
        self.orders_handler = Some(handler);
        self
    }

    pub fn build(&self) -> ZeroExApi {
        ZeroExApi {
            orders_handler: self
                .orders_handler
                .clone()
                .unwrap_or_else(|| self.not_implemented_handler()),
        }
    }

    fn not_implemented_handler(&self) -> OrdersHandler {
        Arc::new(Box::new(|_query: &OrdersQuery| {
            Err(ZeroExResponseError::ServerError(
                "not implemented".to_string(),
            ))
        }))
    }
}

pub struct ZeroExApi {
    orders_handler: OrdersHandler,
}

impl ZeroExApi {
    pub fn builder() -> ZeroExApiBuilder {
        ZeroExApiBuilder::default()
    }

    /// Starts the server and returns the assigned port number.
    pub async fn run(&self) -> u16 {
        let orders_handler = self.orders_handler.clone();

        let orders_route = warp::path("/orderbook/v1/orders")
            .and(warp::query::<HashMap<String, String>>())
            .map(move |params: HashMap<String, String>| {
                let query = OrdersQuery {
                    taker: params.get("taker").and_then(|t| H160::from_str(t).ok()),
                    sender: params.get("sender").and_then(|s| H160::from_str(s).ok()),
                    verifying_contract: params
                        .get("verifyingContract")
                        .and_then(|vc| H160::from_str(vc).ok()),
                };

                match orders_handler(&query) {
                    Ok(orders) => warp::reply::json(&orders).into_response(),
                    Err(err) => warp::reply::with_status(
                        warp::reply::json(&err.to_string()),
                        warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                    )
                    .into_response(),
                }
            });

        let addr: SocketAddr = ([0, 0, 0, 0], 0).into();
        let server = warp::serve(orders_route);
        let (addr, server) = server.bind_ephemeral(addr);
        let port = addr.port();
        assert!(port > 0, "assigned port must be greater than 0");

        tokio::spawn(async move {
            server.await;
        });

        port
    }
}
