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
    router
        .route("/quote", axum::routing::get(get))
        .route("/quote", axum::routing::post(post))
}

async fn get(
    state: axum::extract::State<State>,
    LoggingQuery(order): LoggingQuery<dto::Order>,
) -> Result<axum::Json<dto::Quote>, (axum::http::StatusCode, axum::Json<Error>)> {
    execute(state, order.into_domain()).await
}

async fn post(
    state: axum::extract::State<State>,
    axum::Json(order): axum::Json<dto::PostOrder>,
) -> Result<axum::Json<dto::Quote>, (axum::http::StatusCode, axum::Json<Error>)> {
    execute(state, order.into_domain()).await
}

async fn execute(
    state: axum::extract::State<State>,
    order: crate::domain::quote::Order,
) -> Result<axum::Json<dto::Quote>, (axum::http::StatusCode, axum::Json<Error>)> {
    let handle_request = async {
        observe::quoting(&order);
        let quote = order
            .quote(
                state.eth(),
                state.solver(),
                state.liquidity(),
                state.tokens(),
                &state.competition().risk_detector,
            )
            .await;
        observe::quoted(state.solver().name(), &order, &quote);
        Ok(axum::response::Json(dto::Quote::new(quote?)))
    };

    handle_request
        .instrument(tracing::info_span!("/quote", solver = %state.solver().name()))
        .await
}
