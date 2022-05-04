//! This is a private, undocumented api which will get replaced when we move the solution
//! competition into the api.

use crate::solver_competition::SolverCompetition;
use anyhow::Result;
use model::solver_competition::SolverCompetitionResponse;
use reqwest::StatusCode;
use std::{convert::Infallible, sync::Arc};
use warp::{Filter, Rejection};

fn request() -> impl Filter<Extract = (u64, SolverCompetitionResponse), Error = Rejection> + Clone {
    warp::post()
        .and(warp::path!("solver_competition" / u64))
        .and(crate::api::extract_payload())
}

pub fn post(
    handler: Arc<SolverCompetition>,
) -> impl Filter<Extract = (super::ApiReply,), Error = Rejection> + Clone {
    request().and_then(move |auction_id: u64, model: SolverCompetitionResponse| {
        let handler = handler.clone();
        async move {
            handler.set(auction_id, model);
            let json = warp::reply::json(&());
            let reply = warp::reply::with_status(json, StatusCode::CREATED);
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
        let handler = Arc::new(handler);
        let filter = post(handler.clone());
        let body = serde_json::to_vec(&SolverCompetitionResponse::default()).unwrap();

        let request_ = request()
            .path("/solver_competition/1")
            .method("POST")
            .header("authorization", "password")
            .body(body.clone());
        let response = request_.filter(&filter).await.unwrap().into_response();
        dbg!(&response);
        assert_eq!(response.status(), StatusCode::CREATED);
        assert!(handler.get(1).is_some());
    }
}
