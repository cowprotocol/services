//! This is a private, undocumented api which will get replaced when we move the solution
//! competition into the api.

use crate::solver_competition::SolverCompetition;
use anyhow::Result;
use model::solver_competition::SolverCompetitionResponse;
use reqwest::StatusCode;
use std::{convert::Infallible, sync::Arc};
use warp::{Filter, Rejection};

fn request(
) -> impl Filter<Extract = (String, u64, SolverCompetitionResponse), Error = Rejection> + Clone {
    warp::post()
        .and(warp::header::<String>("Authorization"))
        .and(warp::path!("solver_competition" / u64))
        .and(crate::api::extract_payload())
}

pub fn post(
    handler: Arc<SolverCompetition>,
    expected_auth: String,
) -> impl Filter<Extract = (super::ApiReply,), Error = Rejection> + Clone {
    let expected_auth = Arc::new(expected_auth);
    request().and_then(
        move |auth: String, auction_id: u64, model: SolverCompetitionResponse| {
            let handler = handler.clone();
            let expected_auth = expected_auth.clone();
            async move {
                let (json, status) = if auth.as_str() == expected_auth.as_str() {
                    handler.set(auction_id, model);
                    (warp::reply::json(&()), StatusCode::OK)
                } else {
                    (super::error("Unauthorized", ""), StatusCode::UNAUTHORIZED)
                };
                let reply = warp::reply::with_status(json, status);
                Result::<_, Infallible>::Ok(reply)
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use warp::{test::request, Reply};

    #[tokio::test]
    async fn test() {
        let handler = SolverCompetition::default();
        let handler = Arc::new(handler);
        let auth = "password";
        let filter = post(handler.clone(), auth.to_string());
        let body = serde_json::to_vec(&SolverCompetitionResponse::default()).unwrap();

        let request_ = request()
            .path("/solver_competition/1")
            .method("POST")
            .header("authorization", "password")
            .body(body.clone());
        let response = request_.filter(&filter).await.unwrap().into_response();
        dbg!(&response);
        assert_eq!(response.status(), StatusCode::OK);
        assert!(handler.get(1).is_some());

        let request_ = request()
            .path("/solver_competition/2")
            .method("POST")
            .header("authorization", "1234")
            .body(body);
        let response = request_.filter(&filter).await.unwrap().into_response();
        dbg!(&response);
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(handler.get(2).is_none());
    }
}
