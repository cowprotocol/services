use {super::Response, tracing::Instrument};

pub(crate) mod dto;

use {crate::domain::solver::Solver, std::sync::Arc};

pub async fn solve(
    state: axum::extract::State<Arc<Solver>>,
    auction: axum::extract::Json<dto::Auction>,
) -> (
    axum::http::StatusCode,
    axum::response::Json<Response<dto::Solutions>>,
) {
    let handle_request = async {
        let auction = match auction.to_domain() {
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

        let solutions = dto::Solutions::from_domain(&solutions);
        (
            axum::http::StatusCode::OK,
            axum::response::Json(Response::Ok(solutions)),
        )
    };

    handle_request
        .instrument(tracing::info_span!("/solve"))
        .await
}
