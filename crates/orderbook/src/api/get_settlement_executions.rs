use {
    crate::{database::Postgres, dto::AuctionId},
    hyper::StatusCode,
    serde::Deserialize,
    std::convert::Infallible,
    warp::{reply, Filter, Rejection},
};

pub fn get_settlement_executions_request(
) -> impl Filter<Extract = ((AuctionId, AuctionId),), Error = Rejection> + Clone {
    warp::path!("v1" / "settlement_executions")
        .and(warp::query::<SettlementQuery>())
        .and_then(|query: SettlementQuery| async move {
            Result::<_, Infallible>::Ok((query.from_auction, query.to_auction))
        })
}

#[derive(Debug, Deserialize)]
pub struct SettlementQuery {
    pub from_auction: AuctionId,
    pub to_auction: AuctionId,
}

pub fn get(db: Postgres) -> impl Filter<Extract = (super::ApiReply,), Error = Rejection> + Clone {
    get_settlement_executions_request().and_then(move |(from_auction, to_auction)| {
        let db = db.clone();
        async move {
            let result = db
                .find_settlement_executions(from_auction, to_auction)
                .await;
            let response = match result {
                Ok(executions) => reply::with_status(reply::json(&executions), StatusCode::OK),
                Err(err) => {
                    tracing::error!(
                        ?err,
                        ?from_auction,
                        ?to_auction,
                        "Failed to fetch settlement executions"
                    );
                    crate::api::internal_error_reply()
                }
            };

            Result::<_, Infallible>::Ok(response)
        }
    })
}
