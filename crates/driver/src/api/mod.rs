use {
    crate::solver::Solver,
    futures::Future,
    nonempty::NonEmpty,
    std::{net::SocketAddr, sync::Arc},
};

pub mod execute;
pub mod info;
pub mod solve;

#[derive(Debug, Clone)]
struct State(Arc<StateInner>);

#[derive(Debug)]
struct StateInner {
    solvers: NonEmpty<Solver>,
}

type Router = axum::Router<State>;

pub async fn serve(
    addr: &SocketAddr,
    shutdown: impl Future<Output = ()> + Send + 'static,
    solvers: NonEmpty<Solver>,
) -> Result<(), hyper::Error> {
    // Add middleware.
    let app = axum::Router::new().layer(
        tower::ServiceBuilder::new()
            .layer(tower_http::limit::RequestBodyLimitLayer::new(32 * 1024))
            .layer(tower_http::trace::TraceLayer::new_for_http()),
    );

    // Add routes.
    let app = solve::route(app);
    let app = info::route(app);

    // Add state.
    let app = app.with_state(State(Arc::new(StateInner { solvers })));

    axum::Server::bind(addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown)
        .await
}
