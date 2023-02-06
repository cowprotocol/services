//! This is a private, undocumented api which will get replaced when we move the
//! solution competition into the api.

use {
    crate::solver_competition::SolverCompetitionStoring,
    model::solver_competition::Request,
    reqwest::StatusCode,
    std::{convert::Infallible, sync::Arc},
    warp::{reply::with_status, Filter, Rejection},
};

fn request() -> impl Filter<Extract = (Option<String>, Request), Error = Rejection> + Clone {
    warp::path!("v1" / "solver_competition")
        .and(warp::post())
        .and(warp::header::optional::<String>("Authorization"))
        // While this is an authenticated endpoint we still want to protect against very large
        // that might originate from bugs.
        .and(warp::body::content_length_limit(1e6 as u64))
        .and(warp::body::json())
}

pub fn post(
    handler: Arc<dyn SolverCompetitionStoring>,
    expected_auth: Option<String>,
) -> impl Filter<Extract = (super::ApiReply,), Error = Rejection> + Clone {
    request().and_then(move |auth, request: Request| {
        let handler = handler.clone();
        let expected_auth = expected_auth.clone();
        async move {
            if expected_auth.is_some() && expected_auth != auth {
                return Result::<_, Infallible>::Ok(with_status(
                    super::error("Unauthorized", ""),
                    StatusCode::UNAUTHORIZED,
                ));
            }

            let result = handler.handle_request(request).await;
            Ok(match result {
                Ok(()) => with_status(warp::reply::json(&()), StatusCode::CREATED),
                Err(err) => {
                    tracing::error!(?err, "post_solver_competition");
                    shared::api::internal_error_reply()
                }
            })
        }
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::solver_competition::MockSolverCompetitionStoring,
        warp::{test::request, Reply},
    };

    #[tokio::test]
    async fn test_no_auth() {
        let mut handler = MockSolverCompetitionStoring::new();
        handler.expect_handle_request().returning(|_| Ok(()));

        let filter = post(Arc::new(handler), None);
        let body = serde_json::to_vec(&Request::default()).unwrap();

        let request = request()
            .path("/v1/solver_competition")
            .method("POST")
            .header("authorization", "password")
            .body(body.clone());
        let response = request.reply(&filter).await;
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn test_auth() {
        let mut handler = MockSolverCompetitionStoring::new();
        handler
            .expect_handle_request()
            .times(1)
            .returning(|_| Ok(()));

        let filter = post(Arc::new(handler), Some("auth".to_string()));
        let body = serde_json::to_vec(&Request::default()).unwrap();

        let request_ = request()
            .path("/v1/solver_competition")
            .method("POST")
            .header("authorization", "wrong")
            .body(body.clone());
        let response = request_.filter(&filter).await.unwrap().into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let request_ = request()
            .path("/v1/solver_competition")
            .method("POST")
            .header("authorization", "auth")
            .body(body);
        let response = request_.reply(&filter).await;
        assert_eq!(response.status(), StatusCode::CREATED);
    }
}
