use {
    crate::{app_data, database::Postgres, orderbook::Orderbook, quoter::QuoteHandler},
    axum::{http::HeaderName, routing::MethodRouter},
    hyper::{Method, Uri},
    shared::{api::ApiReply, price_estimation::native::NativePriceEstimating},
    std::sync::Arc,
    tower_http::cors::{AllowHeaders, AllowOrigin},
};

mod cancel_order;
mod cancel_orders;
mod get_app_data;
mod get_auction;
mod get_native_price;
mod get_order_by_uid;
mod get_order_status;
mod get_orders_by_tx;
mod get_solver_competition;
mod get_total_surplus;
mod get_trades;
mod get_user_orders;
mod post_order;
mod post_quote;
mod put_app_data;
mod version;

#[derive(Clone)]
pub struct State {
    database: Postgres,
    orderbook: Arc<Orderbook>,
    quoter: Arc<QuoteHandler>,
    app_data: Arc<app_data::Registry>,
    native_price_estimator: Arc<dyn NativePriceEstimating>,
}

pub fn build_router(
    database: Postgres,
    orderbook: Arc<Orderbook>,
    quotes: Arc<QuoteHandler>,
    app_data: Arc<app_data::Registry>,
    native_price_estimator: Arc<dyn NativePriceEstimating>,
) -> axum::Router<()> {
    axum::Router::new()
       // PROBLEM: we have multiple identical paths with different methods
       // how do we differentiate for the metrics?
       .route_tupled(version::route())
       .route_tupled(post_order::route())
       .route_tupled(cancel_orders::route())
       .route_tupled(get_order_by_uid::route())
       .route_tupled(cancel_order::route())
       .route_tupled(get_order_status::route())
       .route_tupled(get_app_data::route())
       .route_tupled(put_app_data::with_hash_route())
       .route_tupled(put_app_data::without_hash_route())
       .route_tupled(get_auction::route())
       .route_tupled(get_native_price::route())
       .route_tupled(get_orders_by_tx::route())
       .route_tupled(get_solver_competition::latest_route())
       .route_tupled(get_solver_competition::by_tx_hash_route())
       .route_tupled(get_solver_competition::by_auction_id_route())
       .route_tupled(get_total_surplus::route())
       .route_tupled(get_trades::route())
       .route_tupled(get_user_orders::route())
       .route_tupled(post_quote::route())
       .fallback(|uri: Uri, payload: Option<axum::extract::Json<serde_json::Value>>| async move {
           tracing::error!(?uri, payload = payload.map(|p| serde_json::to_string(&p.0).unwrap()), "fallback handler");
       })
       // TRIPLE CHECK THAT THESE LAYERS WORK AS BEFORE!!
       // also does ordering make a difference here?
       .layer(tower_http::trace::TraceLayer::new_for_http())
       .layer(tower_http::cors::CorsLayer::new()
            .allow_methods([
                 Method::GET,
                 Method::POST,
                 Method::DELETE,
                 Method::OPTIONS,
                 Method::PUT,
                 Method::PATCH
            ])
            .allow_headers(AllowHeaders::list([
                HeaderName::from_static("origin"),
                HeaderName::from_static("content-type"),
                HeaderName::from_static("x-auth-token"),
                HeaderName::from_static("x-appid"),
            ]))
            .allow_origin(AllowOrigin::any())
        )
        .layer(axum_prometheus::PrometheusMetricLayer::new())
        .with_state(State {
            database,
            orderbook,
            quoter: quotes,
            app_data,
            native_price_estimator,
        })
    // TODO fallback handler
    //     do we have to add one?
    //     does it set CORS headers correctly?
    // TODO see if the existing unit tests for the endpoints can be converted
    // TODO move api helper function into orderbook??
    // TODO improve error conversions?
    // TODO get rid of unwraps when serializing JSON
    // TODO test timing
    // TODO test tracing
    // TODO test CORS
    // TODO add log prefix
    // TODO add rejection handler
    // TODO double check internal server error handling
    //     currently tests return some errors and I'm not sure if they should be
    // logged     as errors as well
}

pub fn with_status(reply: serde_json::Value, status: axum::http::StatusCode) -> ApiReply {
    ApiReply { status, reply }
}

trait RouterExt<S> {
    /// Pass `path` and `handler` via a tuple which allows defining the handler
    /// together with the path in a separate file which makes reveiwing easier.
    /// Additionally that way types needed parse the request (e.g. JSON payload)
    /// can stay private to the module.
    fn route_tupled(self, route: (&str, MethodRouter<S>)) -> Self;
}

impl<S: Clone + Send + Sync + 'static> RouterExt<S> for axum::Router<S> {
    fn route_tupled(self, (path, handler): (&str, MethodRouter<S>)) -> Self {
        self.route(path, handler)
    }
}
