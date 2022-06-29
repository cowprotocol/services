mod execute;
mod solve;

use futures::Future;
use shared::api::finalize_router;
use std::net::SocketAddr;
use tokio::{task, task::JoinHandle};
use warp::{Filter, Rejection, Reply};

pub fn serve_api(
    address: SocketAddr,
    shutdown_receiver: impl Future<Output = ()> + Send + 'static,
) -> JoinHandle<()> {
    let filter = handle_all_routes().boxed();
    tracing::info!(%address, "serving driver");
    let (_, server) = warp::serve(filter).bind_with_graceful_shutdown(address, shutdown_receiver);
    task::spawn(server)
}

fn handle_all_routes() -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    // Routes for api v1.

    // Note that we add a string with endpoint's name to all responses.
    // This string will be used later to report metrics.
    // It is not used to form the actual server response.

    let solve = solve::post_solve().map(|result| (result, "solve")).boxed();
    let execute = execute::post_execute()
        .map(|result| (result, "execute"))
        .boxed();

    let routes = warp::path!("api" / ..)
        .and(solve.or(execute).unify())
        .untuple_one()
        .boxed();

    finalize_router(routes, "driver::api::request_summary")
}
