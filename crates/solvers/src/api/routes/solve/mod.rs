use {super::Response, tracing::Instrument};

mod dto;

use {crate::domain::solver::Solver, std::sync::Arc};

/// Solve the passed in auction instance.
#[utoipa::path(
    post,
    path = "/solve",
    request_body = Auction,
    responses(
        (status = 200, description = "Auction successfully solved.", body = inline(dto::Solutions)),
        (status = 400, description = "There is something wrong with the request."),
        (status = 429, description = "The solver cannot keep up. It is too busy to handle more requests."),
        (status = 500, description = "Something went wrong when handling the request.")
    ),
)]
pub async fn solve(
    state: axum::extract::State<Arc<Solver>>,
    auction: axum::extract::Json<dto::Auction>,
) -> (
    axum::http::StatusCode,
    axum::response::Json<Response<dto::Solutions>>,
) {
    tracing::info!("newlog received auction={:?}", auction);
    let handle_request = async {
        let auction = match dto::auction::to_domain(&auction) {
            Ok(value) => value,
            Err(err) => {
                tracing::warn!(?err, "invalid auction");
                return (
                    axum::http::StatusCode::BAD_REQUEST,
                    axum::response::Json(Response::Err(err)),
                );
            }
        };

        let auction_id = auction.id;
        let solutions = state
            .solve(auction)
            .instrument(tracing::info_span!("auction", id = %auction_id))
            .await;

        tracing::trace!(?auction_id, ?solutions);

        let solutions = dto::solution::from_domain(&solutions);
        (
            axum::http::StatusCode::OK,
            axum::response::Json(Response::Ok(solutions)),
        )
    };

    handle_request
        .instrument(tracing::info_span!("/solve"))
        .await
}
