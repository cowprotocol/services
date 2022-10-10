use crate::solver_competition::{Identifier, LoadSolverCompetitionError, SolverCompetitionStoring};
use anyhow::Result;
use model::auction::AuctionId;
use primitive_types::H256;
use reqwest::StatusCode;
use shared::api::{convert_json_response, IntoWarpReply};
use std::{convert::Infallible, sync::Arc};
use warp::{reply::with_status, Filter, Rejection};

fn request_id() -> impl Filter<Extract = (Identifier,), Error = Rejection> + Clone {
    warp::path!("solver_competition" / AuctionId)
        .and(warp::get())
        .map(Identifier::Id)
}

fn request_hash() -> impl Filter<Extract = (Identifier,), Error = Rejection> + Clone {
    warp::path!("solver_competition" / "by_tx_hash" / H256)
        .and(warp::get())
        .map(Identifier::Transaction)
}

pub fn get(
    handler: Arc<dyn SolverCompetitionStoring>,
) -> impl Filter<Extract = (super::ApiReply,), Error = Rejection> + Clone {
    request_id()
        .or(request_hash())
        .unify()
        .and_then(move |identifier: Identifier| {
            let handler = handler.clone();
            async move {
                let result = handler.load(identifier).await;
                Result::<_, Infallible>::Ok(convert_json_response(result))
            }
        })
}

impl IntoWarpReply for LoadSolverCompetitionError {
    fn into_warp_reply(self) -> shared::api::ApiReply {
        match self {
            Self::NotFound => with_status(super::error("NotFound", ""), StatusCode::NOT_FOUND),
            Self::Other(err) => err.into_warp_reply(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solver_competition::MockSolverCompetitionStoring;
    use warp::{test::request, Reply};

    #[tokio::test]
    async fn test() {
        let mut storage = MockSolverCompetitionStoring::new();
        storage
            .expect_load()
            .times(2)
            .returning(|_| Ok(Default::default()));
        storage
            .expect_load()
            .times(1)
            .return_once(|_| Err(LoadSolverCompetitionError::NotFound));
        let filter = get(Arc::new(storage));

        let request_ = request().path("/solver_competition/0").method("GET");
        let response = request_.filter(&filter).await.unwrap().into_response();
        dbg!(&response);
        assert_eq!(response.status(), StatusCode::OK);

        let request_ = request().path("/solver_competition/by_tx_hash/0xd51f28edffcaaa76be4a22f6375ad289272c037f3cc072345676e88d92ced8b5").method("GET");
        let response = request_.filter(&filter).await.unwrap().into_response();
        dbg!(&response);
        assert_eq!(response.status(), StatusCode::OK);

        let request_ = request().path("/solver_competition/1337").method("GET");
        let response = request_.filter(&filter).await.unwrap().into_response();
        dbg!(&response);
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
