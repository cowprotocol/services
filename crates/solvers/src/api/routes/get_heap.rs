use axum::{
    body::Bytes,
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
};

pub async fn get_heap() -> impl IntoResponse {
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
    if let Ok(content_type) = "application/octet-stream".parse() {
        headers.insert(header::CONTENT_TYPE, content_type);
    } else {
        tracing::error!("Failed to parse content type header");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to build response headers",
        )
            .into_response();
    }

    if let Ok(content_disposition) = "attachment; filename=\"heap.pprof\"".parse() {
        headers.insert(header::CONTENT_DISPOSITION, content_disposition);
    } else {
        tracing::error!("Failed to parse content disposition header");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to build response headers",
        )
            .into_response();
    }

    (headers, Bytes::from(pprof)).into_response()
}
