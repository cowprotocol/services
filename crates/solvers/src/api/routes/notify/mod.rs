use {crate::domain::solver::Solver, std::sync::Arc, tracing::Instrument};

mod dto;

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
