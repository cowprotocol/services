use {
    crate::solver_competition::{Identifier, LoadSolverCompetitionError, SolverCompetitionStoring},
    anyhow::Result,
    model::{auction::AuctionId, solver_competition::SolverCompetitionAPI},
    primitive_types::H256,
    reqwest::StatusCode,
    std::{convert::Infallible, sync::Arc},
    warp::{
        reply::{with_status, Json, WithStatus},
        Filter,
        Rejection,
    },
};

fn request_id() -> impl Filter<Extract = (Identifier,), Error = Rejection> + Clone {
    warp::path!("v1" / "solver_competition" / AuctionId)
        .and(warp::get())
        .map(Identifier::Id)
}

fn request_hash() -> impl Filter<Extract = (Identifier,), Error = Rejection> + Clone {
    warp::path!("v1" / "solver_competition" / "by_tx_hash" / H256)
        .and(warp::get())
        .map(Identifier::Transaction)
}

fn request_latest() -> impl Filter<Extract = (), Error = Rejection> + Clone {
    warp::path!("v1" / "solver_competition" / "latest").and(warp::get())
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
                let result = handler.load_competition(identifier).await;
                Result::<_, Infallible>::Ok(response(result))
            }
        })
}

pub fn get_latest(
    handler: Arc<dyn SolverCompetitionStoring>,
) -> impl Filter<Extract = (super::ApiReply,), Error = Rejection> + Clone {
    request_latest().and_then(move || {
        let handler = handler.clone();
        async move {
            let result = handler.load_latest_competition().await;
            Result::<_, Infallible>::Ok(response(result))
        }
    })
}

fn response(
    result: Result<SolverCompetitionAPI, crate::solver_competition::LoadSolverCompetitionError>,
) -> WithStatus<Json> {
    match result {
        Ok(response) => with_status(warp::reply::json(&response), StatusCode::OK),
        Err(LoadSolverCompetitionError::NotFound) => with_status(
            super::error("NotFound", "no competition found"),
            StatusCode::NOT_FOUND,
        ),
        Err(LoadSolverCompetitionError::Other(err)) => {
            tracing::error!(?err, "load solver competition");
            shared::api::internal_error_reply()
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::solver_competition::MockSolverCompetitionStoring,
        warp::{test::request, Reply},
    };

    #[tokio::test]
    async fn test() {
        let mut storage = MockSolverCompetitionStoring::new();
        storage
            .expect_load_competition()
            .times(2)
            .returning(|_| Ok(Default::default()));
        storage
            .expect_load_competition()
            .times(1)
            .return_once(|_| Err(LoadSolverCompetitionError::NotFound));
        let filter = get(Arc::new(storage));

        let request_ = request().path("/v1/solver_competition/0").method("GET");
        let response = request_.filter(&filter).await.unwrap().into_response();
        dbg!(&response);
        assert_eq!(response.status(), StatusCode::OK);

        let request_ = request()
            .path(
                "/v1/solver_competition/by_tx_hash/\
                 0xd51f28edffcaaa76be4a22f6375ad289272c037f3cc072345676e88d92ced8b5",
            )
            .method("GET");
        let response = request_.filter(&filter).await.unwrap().into_response();
        dbg!(&response);
        assert_eq!(response.status(), StatusCode::OK);

        let request_ = request().path("/v1/solver_competition/1337").method("GET");
        let response = request_.filter(&filter).await.unwrap().into_response();
        dbg!(&response);
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
