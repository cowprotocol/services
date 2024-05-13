use {crate::domain::solver::Solver, std::sync::Arc, tracing::Instrument};

mod dto;

/// Receive a status notification about a previously provided solution.
#[utoipa::path(
    post,
    path = "/notify",
    request_body = inline(dto::Notification),
    responses(
        (status = 200, description = "Notification successfully received."),
    ),
)]
pub async fn notify(
    state: axum::extract::State<Arc<Solver>>,
    notification: axum::extract::Json<dto::Notification>,
) -> axum::http::StatusCode {
    let handle_request = async {
        let notification = dto::to_domain(&notification);
        let auction_id = notification.auction_id;

        tracing::trace!(?auction_id, ?notification);
        state.notify(notification);

        axum::http::StatusCode::OK
    };

    handle_request
        .instrument(tracing::info_span!("/notify"))
        .await
}
