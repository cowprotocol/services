mod dto;

use {
    crate::{
        domain::{competition, competition::auction},
        infra::{
            api::{self, Error, State},
            observe,
        },
    },
    std::sync::atomic::{AtomicUsize, Ordering},
    tracing::Instrument,
};

pub(in crate::infra::api) fn settle(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/settle", axum::routing::post(route))
}

static REQUEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

async fn route(
    state: axum::extract::State<State>,
    req: axum::Json<dto::SettleRequest>,
) -> Result<(), (hyper::StatusCode, axum::Json<Error>)> {
    let auction_id =
        auction::Id::try_from(req.auction_id).map_err(api::routes::AuctionError::from)?;
    let solver = state.solver().name().to_string();

    let count = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);

    if count < 4 {
        tracing::warn!("Rejecting request {} out of 4", count);
        return Err(competition::Error::SubmissionError.into());
    }

    async move {
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
    .instrument(tracing::info_span!("/settle", solver, %auction_id))
    .await
}
