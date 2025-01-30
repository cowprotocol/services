mod dto;

pub use dto::NotifyError;

use crate::infra::api::{Error, State};

pub(in crate::infra::api) fn notify(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/notify", axum::routing::post(route))
}

async fn route(
    state: axum::extract::State<State>,
    req: axum::Json<dto::NotifyRequest>,
) -> Result<hyper::StatusCode, (hyper::StatusCode, axum::Json<Error>)> {
    let solver = &state.solver().name().0;
    tracing::trace!(?req, ?solver, "received a notification");
    state
        .solver()
        .notify(None, None, req.0.into())
        .await
        .map(|_| hyper::StatusCode::OK)
        .map_err(|_| NotifyError::UnableToNotify.into())
}
