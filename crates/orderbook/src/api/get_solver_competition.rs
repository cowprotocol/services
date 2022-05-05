use crate::solver_competition::SolverCompetition;
use anyhow::Result;
use reqwest::StatusCode;
use std::{convert::Infallible, sync::Arc};
use warp::{Filter, Rejection};

fn request() -> impl Filter<Extract = (u64,), Error = Rejection> + Clone {
    warp::path!("solver_competition" / u64).and(warp::get())
}

pub fn get(
    handler: Arc<SolverCompetition>,
) -> impl Filter<Extract = (super::ApiReply,), Error = Rejection> + Clone {
    request().and_then(move |auction_id| {
        let handler = handler.clone();
        async move {
            let (json, status) = match handler.get(auction_id) {
                Some(response) => (warp::reply::json(&response), StatusCode::OK),
                None => (super::error("NotFound", ""), StatusCode::NOT_FOUND),
            };
            let reply = warp::reply::with_status(json, status);
            Result::<_, Infallible>::Ok(reply)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use warp::{test::request, Reply};

    #[tokio::test]
    async fn test() {
        let handler = SolverCompetition::default();
        handler.set(0, Default::default());
        let filter = get(Arc::new(handler));

        let request_ = request().path("/solver_competition/0").method("GET");
        let response = request_.filter(&filter).await.unwrap().into_response();
        dbg!(&response);
        assert_eq!(response.status(), StatusCode::OK);

        let request_ = request().path("/solver_competition/1").method("GET");
        let response = request_.filter(&filter).await.unwrap().into_response();
        dbg!(&response);
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
