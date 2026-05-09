use {
    crate::infra::{
        api::{Error, State, extract::LoggingQuery},
        observe,
    },
    tracing::Instrument,
};

mod dto;

pub use dto::OrderError;

use crate::domain::{
    competition::order::app_data::AppData,
    quote::{self, QuotingFailed},
};

pub(in crate::infra::api) fn quote(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/quote", axum::routing::get(route))
}

async fn route(
    state: axum::extract::State<State>,
    LoggingQuery(order): LoggingQuery<dto::Order>,
) -> Result<axum::Json<dto::Quote>, (axum::http::StatusCode, axum::Json<Error>)> {
    let handle_request = async {
        let app_data = match order.app_data_hash() {
            Some(app_data_hash) => match state.app_data_retriever() {
                Some(retriever) => match retriever.get_cached_or_fetch(&app_data_hash).await {
                    Ok(Some(app_data)) => AppData::Full(app_data),
                    Ok(None) => AppData::Hash(app_data_hash),
                    Err(err) => {
                        tracing::error!(?app_data_hash, ?err, "failed to fetch quote app data");
                        return Err(quote::Error::from(QuotingFailed::UnsupportedToken).into());
                    }
                },
                None => AppData::Hash(app_data_hash),
            },
            None => AppData::default(),
        };
        let order = order.into_domain(app_data);
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
