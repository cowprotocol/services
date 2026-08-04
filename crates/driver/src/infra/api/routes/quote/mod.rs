use {
    crate::infra::{
        api::{Error, State, extract::LoggingQuery},
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
    LoggingQuery(order): LoggingQuery<dto::Order>,
) -> Result<axum::Json<dto::Quote>, (axum::http::StatusCode, axum::Json<Error>)> {
    let handle_request = async {
        let order = order.into_domain();
        observe::quoting(&order);
        let result = order
            .quote(
                state.eth(),
                state.solver(),
                state.liquidity(),
                state.tokens(),
                state.competition(),
            )
            .await;
        observe::quoted(state.solver().name(), &order, &result);
        let (quote, solution_id) = result?;
        let auction_id = solution_id.and(order.auction_id);
        Ok(axum::response::Json(dto::Quote::new(
            quote,
            state.solver().fast_path_enabled(),
            solution_id,
            auction_id,
        )))
    };

    handle_request
        .instrument(tracing::info_span!("/quote", solver = %state.solver().name()))
        .await
}
