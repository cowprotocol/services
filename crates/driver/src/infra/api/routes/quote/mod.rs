use {
    crate::infra::{
        api::{Error, State},
        observe,
    },
    tap::TapFallible,
    tracing::Instrument,
};

mod dto;

pub use dto::OrderError;

pub(in crate::infra::api) fn quote(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/quote", axum::routing::post(route))
}

async fn route(
    state: axum::extract::State<State>,
    order: axum::Json<dto::Order>,
) -> Result<axum::Json<dto::Quote>, (hyper::StatusCode, axum::Json<Error>)> {
    let handle_request = async {
        let order = order.0.into_domain().tap_err(|err| {
            observe::invalid_dto(err, "order");
        })?;
        observe::quoting(&order);
        let quote = order
            .quote(state.eth(), state.solver(), state.liquidity())
            .await;
        observe::quoted(state.solver().name(), &order, &quote);
        Ok(axum::response::Json(dto::Quote::new(&quote?)))
    };

    handle_request
        .instrument(tracing::info_span!("/quote", solver = %state.solver().name()))
        .await
}
