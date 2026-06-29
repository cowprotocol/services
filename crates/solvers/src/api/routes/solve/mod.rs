use {
    super::Response,
    crate::domain::solver::Solver,
    futures::TryStreamExt,
    std::{io, sync::Arc},
    tokio_util::io::{StreamReader, SyncIoBridge},
    tracing::Instrument,
};

mod dto;

/// `serde_json` reads from the body one byte at a time, and every read on the
/// streaming bridge blocks on the async body. Buffering amortizes that into a
/// handful of reads per request.
const PARSE_BUFFER_SIZE: usize = 256 * 1024;

pub async fn solve(
    state: axum::extract::State<Arc<Solver>>,
    request: axum::extract::Request,
) -> (
    axum::http::StatusCode,
    axum::response::Json<Response<dto::SolverResponse>>,
) {
    let handle_request = async {
        // Stream the request body straight into the JSON parser instead of
        // buffering the whole (potentially multi-MB) auction in memory first.
        // Parsing happens on a blocking thread because `serde_json` is
        // synchronous and CPU-heavy for large auctions.
        let reader = SyncIoBridge::new(StreamReader::new(
            request
                .into_body()
                .into_data_stream()
                .map_err(io::Error::other),
        ));
        let parsed = tokio::task::spawn_blocking(move || {
            serde_json::from_reader::<_, dto::Auction>(io::BufReader::with_capacity(
                PARSE_BUFFER_SIZE,
                reader,
            ))
        })
        .await;
        let auction = match parsed {
            Ok(Ok(auction)) => auction,
            Ok(Err(err)) => {
                tracing::warn!(?err, "failed to deserialize auction");
                return (
                    axum::http::StatusCode::BAD_REQUEST,
                    axum::response::Json(Response::Err("failed to deserialize auction".into())),
                );
            }
            Err(err) => {
                tracing::error!(?err, "auction deserialization task failed");
                return (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    axum::response::Json(Response::Err("internal error".into())),
                );
            }
        };

        let auction = match dto::auction::into_domain(auction) {
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
