use {
    crate::infra::api::State,
    axum::{
        body::Bytes,
        http::{HeaderMap, StatusCode, header},
        response::IntoResponse,
    },
};

pub(in crate::infra::api) fn get_heap(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/get_heap", axum::routing::get(route))
}

async fn route() -> impl IntoResponse {
    let mut prof_ctl = match jemalloc_pprof::PROF_CTL.as_ref() {
        Some(ctl) => ctl.lock().await,
        None => {
            tracing::error!("Profiling not enabled");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Profiling not enabled").into_response();
        }
    };

    let pprof = match prof_ctl.dump_pprof() {
        Ok(data) => data,
        Err(err) => {
            tracing::error!(?err, "Failed to generate heap profile");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to generate heap profile",
            )
                .into_response();
        }
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        "application/octet-stream".parse().unwrap(),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        "attachment; filename=\"heap.pprof\"".parse().unwrap(),
    );

    (headers, Bytes::from(pprof)).into_response()
}
