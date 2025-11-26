use {
    crate::{
        database::Postgres,
        solver_competition::{Identifier, LoadSolverCompetitionError},
    },
    alloy::primitives::B256,
    anyhow::Result,
    model::{AuctionId, solver_competition_v2::Response},
    reqwest::StatusCode,
    std::convert::Infallible,
    warp::{
        Filter,
        Rejection,
        reply::{Json, WithStatus, with_status},
    },
};

fn request_id() -> impl Filter<Extract = (Identifier,), Error = Rejection> + Clone {
    warp::path!("v2" / "solver_competition" / AuctionId)
        .and(warp::get())
        .map(Identifier::Id)
}

fn request_hash() -> impl Filter<Extract = (Identifier,), Error = Rejection> + Clone {
    warp::path!("v2" / "solver_competition" / "by_tx_hash" / B256)
        .and(warp::get())
        .map(Identifier::Transaction)
}

fn request_latest() -> impl Filter<Extract = (), Error = Rejection> + Clone {
    warp::path!("v2" / "solver_competition" / "latest").and(warp::get())
}

pub fn get(db: Postgres) -> impl Filter<Extract = (super::ApiReply,), Error = Rejection> + Clone {
    request_id()
        .or(request_hash())
        .unify()
        .and_then(move |identifier: Identifier| {
            let db = db.clone();
            async move {
                let result = match identifier {
                    Identifier::Id(id) => db.load_competition_by_id(id).await,
                    Identifier::Transaction(hash) => db.load_competition_by_tx_hash(hash).await,
                };
                Result::<_, Infallible>::Ok(response(result))
            }
        })
}

pub fn get_latest(
    db: Postgres,
) -> impl Filter<Extract = (super::ApiReply,), Error = Rejection> + Clone {
    request_latest().and_then(move || {
        let db = db.clone();
        async move {
            let result = db.load_latest_competition().await;
            Result::<_, Infallible>::Ok(response(result))
        }
    })
}

fn response(
    result: Result<Response, crate::solver_competition::LoadSolverCompetitionError>,
) -> WithStatus<Json> {
    match result {
        Ok(response) => with_status(warp::reply::json(&response), StatusCode::OK),
        Err(LoadSolverCompetitionError::NotFound) => with_status(
            super::error("NotFound", "no competition found"),
            StatusCode::NOT_FOUND,
        ),
        Err(LoadSolverCompetitionError::Other(err)) => {
            tracing::error!(?err, "load solver competition");
            crate::api::internal_error_reply()
        }
    }
}
