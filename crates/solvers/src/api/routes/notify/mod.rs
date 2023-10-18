use {crate::domain::solver::Solver, std::sync::Arc, tracing::Instrument};

pub async fn notify(_: axum::extract::State<Arc<Solver>>) -> axum::http::StatusCode {
    let handle_request = async {
        tracing::trace!("request received");
        // todo
        axum::http::StatusCode::OK
    };

    handle_request
        .instrument(tracing::info_span!("/notify"))
        .await
}
