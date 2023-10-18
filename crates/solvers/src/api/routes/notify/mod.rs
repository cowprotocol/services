use {crate::domain::solver::Solver, std::sync::Arc, tracing::Instrument};

mod dto;

pub async fn notify(
    state: axum::extract::State<Arc<Solver>>,
    notification: axum::extract::Json<dto::Notification>,
) -> axum::http::StatusCode {
    let handle_request = async {
        let notification = notification.to_domain();
        let auction_id = notification.auction_id;

        tracing::info!(id = %auction_id, "auction");
        state.notify(notification);

        axum::http::StatusCode::OK
    };

    handle_request
        .instrument(tracing::info_span!("/notify"))
        .await
}
