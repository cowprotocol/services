use {
    super::with_status,
    crate::solver_competition::{Identifier, LoadSolverCompetitionError, SolverCompetitionStoring},
    anyhow::Result,
    axum::{http::StatusCode, routing::MethodRouter},
    model::{auction::AuctionId, solver_competition::SolverCompetitionAPI},
    primitive_types::H256,
    shared::api::{error, ApiReply},
};

pub fn latest_route() -> (&'static str, MethodRouter<super::State>) {
    (LATEST_ENDPOINT, axum::routing::get(latest_handler))
}

const LATEST_ENDPOINT: &str = "/api/v1/solver_competition/latest";
async fn latest_handler(state: axum::extract::State<super::State>) -> ApiReply {
    let result = state.database.load_latest_competition().await;
    response(result)
}

pub fn by_auction_id_route() -> (&'static str, MethodRouter<super::State>) {
    (
        BY_AUCTION_ID_ENDPOINT,
        axum::routing::get(by_auction_id_handler),
    )
}

const BY_AUCTION_ID_ENDPOINT: &str = "/api/v1/solver_competition/:auction_id";
async fn by_auction_id_handler(
    state: axum::extract::State<super::State>,
    auction_id: axum::extract::Path<AuctionId>,
) -> ApiReply {
    let result = state
        .database
        .load_competition(Identifier::Id(auction_id.0))
        .await;
    response(result)
}

pub fn by_tx_hash_route() -> (&'static str, MethodRouter<super::State>) {
    (BY_TX_HASH_ENDPOINT, axum::routing::get(by_tx_hash_handler))
}

const BY_TX_HASH_ENDPOINT: &str = "/api/v1/solver_competition/by_tx_hash/:tx_hash";
async fn by_tx_hash_handler(
    state: axum::extract::State<super::State>,
    tx_hash: axum::extract::Path<H256>,
) -> ApiReply {
    let result = state
        .database
        .load_competition(Identifier::Transaction(tx_hash.0))
        .await;
    response(result)
}

fn response(
    result: Result<SolverCompetitionAPI, crate::solver_competition::LoadSolverCompetitionError>,
) -> ApiReply {
    match result {
        Ok(response) => with_status(serde_json::to_value(&response).unwrap(), StatusCode::OK),
        Err(LoadSolverCompetitionError::NotFound) => with_status(
            error("NotFound", "no competition found"),
            StatusCode::NOT_FOUND,
        ),
        Err(LoadSolverCompetitionError::Other(err)) => {
            tracing::error!(?err, "load solver competition");
            shared::api::internal_error_reply()
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use {
//         super::*,
//         crate::solver_competition::MockSolverCompetitionStoring,
//         warp::{test::request, Reply},
//     };

//     #[tokio::test]
//     async fn test() {
//         let mut storage = MockSolverCompetitionStoring::new();
//         storage
//             .expect_load_competition()
//             .times(2)
//             .returning(|_| Ok(Default::default()));
//         storage
//             .expect_load_competition()
//             .times(1)
//             .return_once(|_| Err(LoadSolverCompetitionError::NotFound));
//         let filter = get(Arc::new(storage));

//         let request_ =
// request().path("/v1/solver_competition/0").method("GET");         let
// response = request_.filter(&filter).await.unwrap().into_response();
//         dbg!(&response);
//         assert_eq!(response.status(), StatusCode::OK);

//         let request_ = request()
//             .path(
//                 "/v1/solver_competition/by_tx_hash/\
//
// 0xd51f28edffcaaa76be4a22f6375ad289272c037f3cc072345676e88d92ced8b5",
//             )
//             .method("GET");
//         let response =
// request_.filter(&filter).await.unwrap().into_response();         dbg!(&
// response);         assert_eq!(response.status(), StatusCode::OK);

//         let request_ =
// request().path("/v1/solver_competition/1337").method("GET");         let
// response = request_.filter(&filter).await.unwrap().into_response();
//         dbg!(&response);
//         assert_eq!(response.status(), StatusCode::NOT_FOUND);
//     }
// }
