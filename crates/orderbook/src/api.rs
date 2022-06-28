mod cancel_order;
mod create_order;
mod get_auction;
mod get_fee_and_quote;
mod get_fee_info;
mod get_markets;
mod get_order_by_uid;
mod get_orders_by_tx;
mod get_solvable_orders;
mod get_solvable_orders_v2;
mod get_solver_competition;
mod get_trades;
mod get_user_orders;
mod post_quote;
mod post_solver_competition;
mod replace_order;

use crate::solver_competition::SolverCompetition;
use crate::{database::trades::TradeRetrieving, order_quoting::QuoteHandler, orderbook::Orderbook};
use shared::api::{error, handle_rejection, internal_error, ApiMetrics, ApiReply};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    sync::Arc,
    time::Instant,
};
use warp::{Filter, Rejection, Reply};

pub fn handle_all_routes(
    database: Arc<dyn TradeRetrieving>,
    orderbook: Arc<Orderbook>,
    quotes: Arc<QuoteHandler>,
    solver_competition: Arc<SolverCompetition>,
    solver_competition_auth: Option<String>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    // Routes for api v1.

    // Note that we add a string with endpoint's name to all responses.
    // This string will be used later to report metrics.
    // It is not used to form the actual server response.

    let create_order = create_order::create_order(orderbook.clone())
        .map(|result| (result, "v1/create_order"))
        .boxed();
    let fee_info = get_fee_info::get_fee_info(quotes.clone())
        .map(|result| (result, "v1/fee_info"))
        .boxed();
    let get_order = get_order_by_uid::get_order_by_uid(orderbook.clone())
        .map(|result| (result, "v1/get_order"))
        .boxed();
    let get_solvable_orders = get_solvable_orders::get_solvable_orders(orderbook.clone())
        .map(|result| (result, "v1/get_solvable_orders"))
        .boxed();
    let get_trades = get_trades::get_trades(database)
        .map(|result| (result, "v1/get_trades"))
        .boxed();
    let cancel_order = cancel_order::cancel_order(orderbook.clone())
        .map(|result| (result, "v1/cancel_order"))
        .boxed();
    let replace_order = replace_order::filter(orderbook.clone())
        .map(|result| (result, "v1/replace_order"))
        .boxed();
    let get_amount_estimate = get_markets::get_amount_estimate(quotes.clone())
        .map(|result| (result, "v1/get_amount_estimate"))
        .boxed();
    let get_fee_and_quote_sell = get_fee_and_quote::get_fee_and_quote_sell(quotes.clone())
        .map(|result| (result, "v1/get_fee_and_quote_sell"))
        .boxed();
    let get_fee_and_quote_buy = get_fee_and_quote::get_fee_and_quote_buy(quotes.clone())
        .map(|result| (result, "v1/get_fee_and_quote_buy"))
        .boxed();
    let get_user_orders = get_user_orders::get_user_orders(orderbook.clone())
        .map(|result| (result, "v1/get_user_orders"))
        .boxed();
    let get_orders_by_tx = get_orders_by_tx::get_orders_by_tx(orderbook.clone())
        .map(|result| (result, "v1/get_orders_by_tx"))
        .boxed();
    let post_quote = post_quote::post_quote(quotes)
        .map(|result| (result, "v1/post_quote"))
        .boxed();
    let get_auction = get_auction::get_auction(orderbook.clone())
        .map(|result| (result, "v1/auction"))
        .boxed();
    let get_solver_competition = get_solver_competition::get(solver_competition.clone())
        .map(|result| (result, "v1/solver_competition"))
        .boxed();
    let post_solver_competition =
        post_solver_competition::post(solver_competition, solver_competition_auth)
            .map(|result| (result, "v1/solver_competition"))
            .boxed();

    let routes_v1 = warp::path!("api" / "v1" / ..)
        .and(
            create_order
                .or(fee_info)
                .unify()
                .or(get_order)
                .unify()
                .or(get_solvable_orders)
                .unify()
                .or(get_trades)
                .unify()
                .or(cancel_order)
                .unify()
                .or(replace_order)
                .unify()
                .or(get_amount_estimate)
                .unify()
                .or(get_fee_and_quote_sell)
                .unify()
                .or(get_fee_and_quote_buy)
                .unify()
                .or(get_user_orders)
                .unify()
                .or(get_orders_by_tx)
                .unify()
                .or(post_quote)
                .unify()
                .or(get_auction)
                .unify()
                .or(get_solver_competition)
                .unify()
                .or(post_solver_competition)
                .unify(),
        )
        .untuple_one()
        .boxed();

    // Routes for api v2.

    let get_solvable_orders_v2 = get_solvable_orders_v2::get_solvable_orders(orderbook)
        .map(|result| (result, "v2/get_solvable_orders"))
        .boxed();

    let routes_v2 = warp::path!("api" / "v2" / ..)
        .and(get_solvable_orders_v2)
        .untuple_one();

    // Routes combined

    let routes = routes_v1.or(routes_v2).unify().boxed();

    // Metrics

    let metrics = ApiMetrics::pub_instance();
    let routes_with_metrics = warp::any()
        .map(Instant::now) // Start a timer at the beginning of response processing
        .and(routes) // Parse requests
        .map(|timer: Instant, reply: ApiReply, method: &str| {
            let response = reply.into_response();

            metrics
                .requests_complete
                .with_label_values(&[method, response.status().as_str()])
                .inc();
            metrics
                .requests_duration_seconds
                .with_label_values(&[method])
                .observe(timer.elapsed().as_secs_f64());

            response
        })
        .boxed();

    // Final setup

    let cors = warp::cors()
        .allow_any_origin()
        .allow_methods(vec!["GET", "POST", "DELETE", "OPTIONS", "PUT", "PATCH"])
        .allow_headers(vec!["Origin", "Content-Type", "X-Auth-Token", "X-AppId"]);

    // Give each request a unique tracing span.
    // This allows us to match log statements across concurrent API requests. We
    // first try to read the request ID from our reverse proxy (this way we can
    // line up API request logs with Nginx requests) but fall back to an
    // internal counter.
    let internal_request_id = Arc::new(AtomicUsize::new(0));
    let tracing_span = warp::trace(move |info| {
        if let Some(header) = info.request_headers().get("X-Request-ID") {
            let request_id = String::from_utf8_lossy(header.as_bytes());
            tracing::info_span!("request", id = &*request_id)
        } else {
            let request_id = internal_request_id.fetch_add(1, Ordering::SeqCst);
            tracing::info_span!("request", id = request_id)
        }
    });

    routes_with_metrics
        .recover(handle_rejection)
        .with(cors)
        .with(warp::log::log("orderbook::api::request_summary"))
        .with(tracing_span)
}
