mod dto;

use {
    crate::{
        domain::competition::auction,
        infra::{
            api::{self, Error, State},
            observe,
        },
    },
    tracing::Instrument,
};

pub(in crate::infra::api) fn reveal(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/reveal", axum::routing::post(route))
}

async fn route(
    state: axum::extract::State<State>,
    req: axum::Json<dto::RevealRequest>,
) -> Result<axum::Json<dto::RevealResponse>, (hyper::StatusCode, axum::Json<Error>)> {
    let auction_id =
        auction::Id::try_from(req.auction_id).map_err(api::routes::AuctionError::from)?;
    let handle_request = async {
        observe::revealing();
        let result = state
            .competition()
            .reveal(req.solution_id, auction_id)
            .await;
        observe::revealed(state.solver().name(), &result);
        let result = result?;
        Ok(axum::Json(dto::RevealResponse::new(result)))
    };

    handle_request
        .instrument(tracing::info_span!("/reveal", solver = %state.solver().name(), %auction_id))
        .await
}
