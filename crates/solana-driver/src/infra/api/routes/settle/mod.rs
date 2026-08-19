//! The `/settle` handler: submit a previously proposed solution.
//!
//! TODO: predicted demo-grade pipeline. Real settlement encoding, simulation,
//! and submission strategies replace the direct build-sign-send here.

use {
    crate::infra::{
        api::{State, dto},
        settlement,
    },
    axum::Json,
};

pub async fn settle(
    state: axum::extract::State<State>,
    Json(request): Json<dto::SettleRequest>,
) -> Result<Json<dto::SettleResponse>, axum::http::StatusCode> {
    let Some(stored) = state.stored_solution(request.auction_id, request.solution_id) else {
        tracing::warn!(
            auction_id = request.auction_id,
            solution_id = request.solution_id,
            "settle references an unknown solution"
        );
        return Err(axum::http::StatusCode::NOT_FOUND);
    };

    match settlement::submit(
        state.rpc(),
        state.keypair(),
        state.settlement_program(),
        request.auction_id,
        &stored.orders,
        &stored.solution,
    )
    .await
    {
        Ok(tx_signature) => Ok(Json(dto::SettleResponse { tx_signature })),
        Err(err) => {
            tracing::error!(?err, "settlement submission failed");
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
