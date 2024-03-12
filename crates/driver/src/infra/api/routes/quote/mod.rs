use {
    crate::infra::{
        api::{Error, State},
        observe,
    },
    tracing::Instrument,
};

mod dto;

pub use dto::OrderError;

pub(in crate::infra::api) fn quote(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/quote", axum::routing::get(route))
}

async fn route(
    state: axum::extract::State<State>,
    order: axum::extract::Query<dto::Order>,
) -> Result<axum::Json<dto::Quote>, (hyper::StatusCode, axum::Json<Error>)> {
    let handle_request = async {
        let order = order.0.into_domain(state.timeouts());
        observe::quoting(&order);
        let quote = order
            .quote(
                state.eth(),
                state.solver(),
                state.liquidity(),
                state.tokens(),
            )
            .await;
        observe::quoted(state.solver().name(), &order, &quote);
        Ok(axum::response::Json(dto::Quote::new(&quote?)))
    };

    handle_request
        .instrument(tracing::info_span!("/quote", solver = %state.solver().name()))
        .await
}
