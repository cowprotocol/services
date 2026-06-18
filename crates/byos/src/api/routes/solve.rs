use {
    crate::api::AppState,
    axum::{Json, extract::State, http::StatusCode, response::IntoResponse},
    solvers_dto::solution::{
        CustomInteraction,
        Fulfillment,
        Interaction,
        OrderUid,
        Solution,
        SolverResponse,
        Trade,
    },
    std::sync::Arc,
    tracing::Instrument,
};

pub async fn solve(
    State(state): State<Arc<AppState>>,
    Json(auction): Json<solvers_dto::auction::Auction>,
) -> impl IntoResponse {
    let handle_request = async {
        let auction_id = auction.id.unwrap_or(-1);
        tracing::info!(%auction_id, orders = auction.orders.len(), "received /solve");

        let mut solutions = Vec::new();

        for (i, order) in auction.orders.iter().enumerate() {
            let Some(proposal) = state.store.get_best(&order.uid).await else {
                continue;
            };

            tracing::debug!(
                order_uid = %const_hex::encode_prefixed(order.uid),
                solver = %proposal.solver,
                sell_amount = %proposal.sell_amount,
                buy_amount = %proposal.buy_amount,
                "matched proposal to order",
            );

            let executed_amount = match order.kind {
                solvers_dto::auction::Kind::Sell => proposal.sell_amount,
                solvers_dto::auction::Kind::Buy => proposal.buy_amount,
            };

            let solution = Solution {
                id: i as u64,
                prices: [
                    (order.sell_token, proposal.buy_amount),
                    (order.buy_token, proposal.sell_amount),
                ]
                .into_iter()
                .collect(),
                trades: vec![Trade::Fulfillment(Fulfillment {
                    order: OrderUid(order.uid),
                    executed_amount,
                    fee: Some(alloy_primitives::U256::ZERO),
                })],
                interactions: proposal
                    .interactions
                    .iter()
                    .map(|i| {
                        Interaction::Custom(CustomInteraction {
                            internalize: false,
                            target: i.target,
                            value: i.value,
                            calldata: i.calldata.clone(),
                            allowances: vec![],
                            inputs: vec![],
                            outputs: vec![],
                        })
                    })
                    .collect(),
                pre_interactions: vec![],
                post_interactions: vec![],
                gas: None,
                gas_fee_override: None,
                flashloans: None,
                wrappers: vec![],
            };

            solutions.push(solution);
        }

        tracing::info!(
            %auction_id,
            matched = solutions.len(),
            "returning solutions",
        );

        (
            StatusCode::OK,
            Json(SolverResponse::Solutions { solutions }),
        )
    };

    handle_request
        .instrument(tracing::info_span!("/solve"))
        .await
}
