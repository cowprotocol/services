mod dto;

use {
    crate::{
        domain::competition::auction,
        infra::{
            api::{self, Error, State, extract},
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
    req: extract::Json<dto::SettleRequest>,
) -> Result<(), (axum::http::StatusCode, axum::Json<Error>)> {
    let req = req.0;
    let auction_id =
        auction::Id::try_from(req.auction_id).map_err(api::routes::AuctionError::from)?;
    let solver = state.solver().name().to_string();

    async move {
        observe::settling();
        let result = state
            .competition()
            .settle(
                auction_id,
                req.solution_id,
                req.submission_deadline_latest_block.into(),
            )
            .await;
        result.map(|_| ()).map_err(Into::into)
    }
    .instrument(tracing::info_span!("/settle", solver, %auction_id))
    .await
}
