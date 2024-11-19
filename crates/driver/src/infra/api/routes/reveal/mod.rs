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
    let competition = state.competition();
    let auction_id = competition.auction_id(req.solution_id).map(|id| id.0);
    let handle_request = async {
        observe::revealing();
        let result = competition.reveal(req.solution_id).await;
        observe::revealed(state.solver().name(), &result);
        let result = result?;
        Ok(axum::Json(dto::Revealed::new(result)))
    };

    handle_request
        .instrument(tracing::info_span!("/reveal", solver = %state.solver().name(), auction_id))
        .await
}
