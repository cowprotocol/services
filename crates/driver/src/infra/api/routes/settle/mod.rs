mod dto;

use {
    crate::{
        domain::competition,
        infra::{
            api::{Error, State},
            observe,
        },
    },
    tracing::Instrument,
};

pub(in crate::infra::api) fn settle(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/settle", axum::routing::post(route))
}

async fn route(
    state: axum::extract::State<State>,
    solution: axum::Json<dto::Solution>,
) -> Result<(), (hyper::StatusCode, axum::Json<Error>)> {
    let state = state.clone();
    let auction_id = state.competition().auction_id().map(|id| id.0);
    let solver = state.solver().name();
    let span = tracing::info_span!("/settle", %solver, auction_id);

    let handle_request = async move {
        observe::settling();
        let result = state
            .competition()
            .settle(solution.submission_deadline_latest_block)
            .await;
        observe::settled(state.solver().name(), &result);
        result.map(|_| ()).map_err(Into::into)
    }
    .instrument(span);

    // Handle `/settle` call in a background task to ensure that we correctly
    // submit the settlement (or cancellation) on-chain even if the server
    // aborts the endpoint handler code.
    // This can happen due do connection issues or when the autopilot aborts
    // the `/settle` call when we reach the submission deadline.
    Ok(tokio::task::spawn(handle_request)
        .await
        .unwrap_or_else(|_| Err(competition::Error::SubmissionError))?)
}
