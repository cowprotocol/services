use {
    reqwest::StatusCode,
    std::convert::Infallible,
    warp::{Filter, Rejection, Reply, reply::with_status},
};

pub fn version() -> impl Filter<Extract = (Box<dyn Reply>,), Error = Rejection> + Clone {
    warp::path!("v1" / "version")
        .and(warp::get())
        .and_then(|| async {
            Result::<_, Infallible>::Ok(Box::new(with_status(
                env!("VERGEN_GIT_DESCRIBE"),
                StatusCode::OK,
            )) as Box<dyn Reply>)
        })
}
