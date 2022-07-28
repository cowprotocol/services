//! This is a private, undocumented api which will get replaced when we move the solution
//! competition into the api.

use crate::solver_competition::SolverCompetitionStoring;
use model::solver_competition::SolverCompetition;
use reqwest::StatusCode;
use shared::api::convert_json_response_with_status;
use std::{convert::Infallible, sync::Arc};
use warp::{reply::with_status, Filter, Rejection};

#[async_trait::async_trait]
pub trait SolvableOrdersCache: Send + Sync {
    async fn update_next_solver_competition_id(&self) -> anyhow::Result<()>;
}

#[async_trait::async_trait]
impl SolvableOrdersCache for crate::solvable_orders::SolvableOrdersCache {
    async fn update_next_solver_competition_id(&self) -> anyhow::Result<()> {
        crate::solvable_orders::SolvableOrdersCache::update_next_solver_competition_id(self).await
    }
}

fn request() -> impl Filter<Extract = (Option<String>, SolverCompetition), Error = Rejection> + Clone
{
    warp::path!("solver_competition")
        .and(warp::post())
        .and(warp::header::optional::<String>("Authorization"))
        // While this is an authenticated endpoint we still want to protect against very large
        // that might originate from bugs.
        .and(warp::body::content_length_limit(1e6 as u64))
        .and(warp::body::json())
}

pub fn post(
    handler: Arc<dyn SolverCompetitionStoring>,
    solvable_orders: Arc<dyn SolvableOrdersCache>,
    expected_auth: Option<String>,
) -> impl Filter<Extract = (super::ApiReply,), Error = Rejection> + Clone {
    request().and_then(move |auth, model: SolverCompetition| {
        let handler = handler.clone();
        let solvable_orders = solvable_orders.clone();
        let expected_auth = expected_auth.clone();
        async move {
            if expected_auth.is_some() && expected_auth != auth {
                return Result::<_, Infallible>::Ok(with_status(
                    super::error("Unauthorized", ""),
                    StatusCode::UNAUTHORIZED,
                ));
            }

            let result = handler.save(model).await;
            // Update the id immediately so that the next driver run cannot observe the id repeating.
            if let Err(err) = solvable_orders.update_next_solver_competition_id().await {
                tracing::warn!(?err, "failed to update next solver competition id");
            }
            Ok(convert_json_response_with_status(
                result,
                StatusCode::CREATED,
            ))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solver_competition::MockSolverCompetitionStoring;
    use warp::{test::request, Reply};

    struct NoopSolvableOrdersCache;
    #[async_trait::async_trait]
    impl SolvableOrdersCache for NoopSolvableOrdersCache {
        async fn update_next_solver_competition_id(&self) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_no_auth() {
        let mut handler = MockSolverCompetitionStoring::new();
        handler.expect_save().returning(|_| Ok(1));
        let cache = Arc::new(NoopSolvableOrdersCache);

        let filter = post(Arc::new(handler), cache, None);
        let body = serde_json::to_vec(&SolverCompetition::default()).unwrap();

        let request = request()
            .path("/solver_competition")
            .method("POST")
            .header("authorization", "password")
            .body(body.clone());
        let response = request.reply(&filter).await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let response: u64 = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(response, 1);
    }

    #[tokio::test]
    async fn test_auth() {
        let mut handler = MockSolverCompetitionStoring::new();
        handler.expect_save().times(1).returning(|_| Ok(1));
        let cache = Arc::new(NoopSolvableOrdersCache);

        let filter = post(Arc::new(handler), cache, Some("auth".to_string()));
        let body = serde_json::to_vec(&SolverCompetition::default()).unwrap();

        let request_ = request()
            .path("/solver_competition")
            .method("POST")
            .header("authorization", "wrong")
            .body(body.clone());
        let response = request_.filter(&filter).await.unwrap().into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let request_ = request()
            .path("/solver_competition")
            .method("POST")
            .header("authorization", "auth")
            .body(body);
        let response = request_.reply(&filter).await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let response: u64 = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(response, 1);
    }
}
