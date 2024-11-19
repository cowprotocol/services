mod dto;

use {
    crate::infra::{
        api::{Error, State},
        observe,
    },
    tracing::Instrument,
};

pub(in crate::infra::api) fn reveal(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/reveal", axum::routing::post(route))
}

async fn route(
    state: axum::extract::State<State>,
    req: axum::Json<dto::Solution>,
) -> Result<axum::Json<dto::Revealed>, (hyper::StatusCode, axum::Json<Error>)> {
    let handle_request = async {
        observe::revealing();
        let result = state
            .competition()
            .reveal(
                req.solution_id,
                req.auction_id
                    .as_ref()
                    .map(|value| {
                        value
                            .parse::<i64>()
                            .map_err(|_| crate::domain::competition::Error::SolutionNotAvailable)
                    })
                    .transpose()?,
            )
            .await;
        observe::revealed(state.solver().name(), &result);
        let result = result?;
        Ok(axum::Json(dto::Revealed::new(result)))
    };

    handle_request
        .instrument(tracing::info_span!("/reveal", solver = %state.solver().name(), req.auction_id))
        .await
}
