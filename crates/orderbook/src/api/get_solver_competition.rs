use crate::solver_competition::{LoadSolverCompetitionError, SolverCompetitionStoring};
use anyhow::Result;
use reqwest::StatusCode;
use shared::api::{convert_json_response, IntoWarpReply};
use std::{convert::Infallible, sync::Arc};
use warp::{reply::with_status, Filter, Rejection};

fn request() -> impl Filter<Extract = (u64,), Error = Rejection> + Clone {
    warp::path!("solver_competition" / u64).and(warp::get())
}

pub fn get(
    handler: Arc<dyn SolverCompetitionStoring>,
) -> impl Filter<Extract = (super::ApiReply,), Error = Rejection> + Clone {
    request().and_then(move |id| {
        let handler = handler.clone();
        async move {
            let result = handler.load(id).await;
            Result::<_, Infallible>::Ok(convert_json_response(result))
        }
    })
}

impl IntoWarpReply for LoadSolverCompetitionError {
    fn into_warp_reply(self) -> shared::api::ApiReply {
        match self {
            Self::NotFound(_) => with_status(super::error("NotFound", ""), StatusCode::NOT_FOUND),
            Self::Other(err) => err.into_warp_reply(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solver_competition::InMemoryStorage;
    use warp::{test::request, Reply};

    #[tokio::test]
    async fn test() {
        let handler = InMemoryStorage::default();
        let id = handler.save(Default::default()).await.unwrap();
        let filter = get(Arc::new(handler));

        let request_ = request()
            .path(&format!("/solver_competition/{id}"))
            .method("GET");
        let response = request_.filter(&filter).await.unwrap().into_response();
        dbg!(&response);
        assert_eq!(response.status(), StatusCode::OK);

        let request_ = request().path("/solver_competition/1337").method("GET");
        let response = request_.filter(&filter).await.unwrap().into_response();
        dbg!(&response);
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
