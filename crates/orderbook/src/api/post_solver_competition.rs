//! This is a private, undocumented api which will get replaced when we move the solution
//! competition into the api.

use crate::solver_competition::SolverCompetition;
use anyhow::Result;
use model::solver_competition::SolverCompetitionResponse;
use reqwest::StatusCode;
use std::{convert::Infallible, sync::Arc};
use warp::{Filter, Rejection};

fn request(
) -> impl Filter<Extract = (u64, Option<String>, SolverCompetitionResponse), Error = Rejection> + Clone
{
    warp::path!("solver_competition" / u64)
        .and(warp::post())
        .and(warp::header::optional::<String>("Authorization"))
        // While this is an authenticated endpoint we still want to protect against very large
        // that might originate from bugs.
        .and(warp::body::content_length_limit(1e6 as u64))
        .and(warp::body::json())
}

pub fn post(
    handler: Arc<SolverCompetition>,
    expected_auth: Option<String>,
) -> impl Filter<Extract = (super::ApiReply,), Error = Rejection> + Clone {
    request().and_then(
        move |auction_id: u64, auth, model: SolverCompetitionResponse| {
            let handler = handler.clone();
            let expected_auth = expected_auth.clone();
            async move {
                let (json, status) = if expected_auth.is_none() || expected_auth == auth {
                    handler.set(auction_id, model);
                    (warp::reply::json(&()), StatusCode::CREATED)
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
    async fn test_no_auth() {
        let handler = SolverCompetition::default();
        let handler = Arc::new(handler);
        let filter = post(handler.clone(), None);
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

    #[tokio::test]
    async fn test_auth() {
        let handler = SolverCompetition::default();
        let handler = Arc::new(handler);
        let filter = post(handler.clone(), Some("auth".to_string()));
        let body = serde_json::to_vec(&SolverCompetitionResponse::default()).unwrap();

        let request_ = request()
            .path("/solver_competition/1")
            .method("POST")
            .header("authorization", "wrong")
            .body(body.clone());
        let response = request_.filter(&filter).await.unwrap().into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(handler.get(1).is_none());

        let request_ = request()
            .path("/solver_competition/1")
            .method("POST")
            .header("authorization", "auth")
            .body(body);
        let response = request_.filter(&filter).await.unwrap().into_response();
        assert_eq!(response.status(), StatusCode::CREATED);
        assert!(handler.get(1).is_some());
    }
}
