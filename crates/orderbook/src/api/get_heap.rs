use {
    std::convert::Infallible,
    warp::{
        Filter,
        Rejection,
        Reply,
        hyper::{Body, Response, StatusCode},
    },
};

pub fn get_heap() -> impl Filter<Extract = (Box<dyn Reply>,), Error = Rejection> + Clone {
    warp::path!("v1" / "get_heap")
        .and(warp::get())
        .and_then(|| async {
            let mut prof_ctl = match jemalloc_pprof::PROF_CTL.as_ref() {
                Some(ctl) => ctl.lock().await,
                None => {
                    tracing::error!("Profiling not enabled");
                    return Result::<_, Infallible>::Ok(Box::new(warp::reply::with_status(
                        "Profiling not enabled",
                        StatusCode::INTERNAL_SERVER_ERROR,
                    )) as Box<dyn Reply>);
                }
            };

            let pprof = match prof_ctl.dump_pprof() {
                Ok(data) => data,
                Err(err) => {
                    tracing::error!(?err, "Failed to generate heap profile");
                    return Result::<_, Infallible>::Ok(Box::new(warp::reply::with_status(
                        "Failed to generate heap profile",
                        StatusCode::INTERNAL_SERVER_ERROR,
                    )) as Box<dyn Reply>);
                }
            };

            let response = match Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/octet-stream")
                .header("Content-Disposition", "attachment; filename=\"heap.pprof\"")
                .body(Body::from(pprof))
            {
                Ok(response) => response,
                Err(err) => {
                    tracing::error!(?err, "Failed to build heap profile response");
                    return Result::<_, Infallible>::Ok(Box::new(warp::reply::with_status(
                        "Failed to build response",
                        StatusCode::INTERNAL_SERVER_ERROR,
                    )) as Box<dyn Reply>);
                }
            };

            Result::<_, Infallible>::Ok(Box::new(response) as Box<dyn Reply>)
        })
}
