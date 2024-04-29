mod dto;

use {
    crate::infra::{
        api::{Error, State},
        observe,
    },
    tracing::Instrument,
};

pub(in crate::infra::api) fn settle(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/settle", axum::routing::post(route))
}

async fn route(
    state: axum::extract::State<State>,
    _: axum::Json<dto::Solution>,
) -> Result<(), (hyper::StatusCode, axum::Json<Error>)> {
    let competition = state.competition();
    let auction_id = competition.auction_id().map(|id| id.0);
    let handle_request = async {
        observe::settling();
        let result = competition.settle().await;
        observe::settled(state.solver().name(), &result);
        match result {
            Err(err) => Err(err.into()),
            Ok(_) => Ok(()),
        }
    };

    handle_request
        .instrument(tracing::info_span!("/settle", solver = %state.solver().name(), auction_id))
        .await
}
