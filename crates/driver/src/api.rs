pub mod execute;
pub mod solve;

use crate::driver::Driver;
use futures::Future;
use shared::api::finalize_router;
use std::{net::SocketAddr, sync::Arc};
use tokio::{task, task::JoinHandle};
use warp::{Filter, Rejection, Reply};

pub fn serve_api(
    address: SocketAddr,
    shutdown_receiver: impl Future<Output = ()> + Send + 'static,
    drivers: Vec<(Arc<Driver>, String)>,
) -> JoinHandle<()> {
    let filter = handle_all_routes(drivers).boxed();
    tracing::info!(%address, "serving driver");
    let (_, server) = warp::serve(filter).bind_with_graceful_shutdown(address, shutdown_receiver);
    task::spawn(server)
}

fn handle_all_routes(
    drivers: Vec<(Arc<Driver>, String)>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    // Routes for api v1.

    // Note that we add a string with endpoint's name to all responses.
    // This string will be used later to report metrics.
    // It is not used to form the actual server response.

    let mut routes = vec![];
    for (driver, name) in drivers.into_iter() {
        // leak string to use it in tracing spans
        let name = Box::leak(name.into_boxed_str());
        routes.push(("solve", solve::post_solve(name, driver.clone()).boxed()));
        routes.push(("execute", execute::post_execute(name, driver).boxed()));
    }

    finalize_router(routes, "driver::api::request_summary")
}
