mod dto;

use {
    crate::{
        domain::{competition, competition::auction},
        infra::{
            api::{self, Error, State},
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
    req: axum::Json<dto::SettleRequest>,
) -> Result<(), (hyper::StatusCode, axum::Json<Error>)> {
    let auction_id = req
        .auction_id
        .map(auction::Id::try_from)
        .transpose()
        .map_err(Into::<api::routes::AuctionError>::into)?;
    let solver = state.solver().name().to_string();

    let handle_request = async move {
        observe::settling();
        let result = state
            .competition()
            .settle(
                auction_id,
                req.solution_id,
                req.submission_deadline_latest_block,
            )
            .await;
        observe::settled(state.solver().name(), &result);
        result.map(|_| ()).map_err(Into::into)
    }
    .instrument(tracing::info_span!("/settle", solver, auction_id = ?auction_id.map(|id| id.0)));

    // Handle `/settle` call in a background task to ensure that we correctly
    // submit the settlement (or cancellation) on-chain even if the server
    // aborts the endpoint handler code.
    // This can happen due do connection issues or when the autopilot aborts
    // the `/settle` call when we reach the submission deadline.
    Ok(
        ::observe::request_id::spawn_task_with_current_request_id(handle_request)
            .await
            .unwrap_or_else(|_| Err(competition::Error::SubmissionError))?,
    )
}
