//! The `/solve` handler: the autopilot's auction in, ranked solutions out.
//!
//! TODO: predicted demo-grade pipeline. The real auction pre-processing
//! (balances, filtering) and solution validation replace the direct mapping
//! here.

use {
    crate::{
        domain,
        infra::api::{State, dto},
    },
    axum::Json,
    std::collections::HashMap,
};

pub async fn solve(
    state: axum::extract::State<State>,
    Json(request): Json<dto::SolveRequest>,
) -> Result<Json<dto::SolveResponse>, axum::http::StatusCode> {
    let deadline = match state.deadline_from_slot(request.deadline_slot).await {
        Ok(deadline) => deadline,
        Err(err) => {
            tracing::warn!(?err, "failed to resolve the auction deadline");
            return Err(axum::http::StatusCode::SERVICE_UNAVAILABLE);
        }
    };
    let auction = domain::Auction {
        id: request.id,
        orders: request
            .orders
            .iter()
            .map(|order| domain::Order {
                uid: order.uid,
                sell_mint: order.sell_token,
                buy_mint: order.buy_token,
                // The engine quotes the exact side of the order.
                amount: match order.kind {
                    dto::Kind::Sell => order.sell_amount,
                    dto::Kind::Buy => order.buy_amount,
                },
                side: match order.kind {
                    dto::Kind::Sell => domain::Side::Sell,
                    dto::Kind::Buy => domain::Side::Buy,
                },
            })
            .collect(),
        deadline,
    };

    let mut solutions = Vec::new();
    for solver in state.solvers() {
        match solver.solve(&auction).await {
            Ok(mut engine_solutions) => solutions.append(&mut engine_solutions),
            Err(err) => tracing::warn!(?err, "engine solve failed"),
        }
    }

    // Re-key the solutions: engine-local ids collide across engines, and
    // `/settle` references the id we answer with.
    let stored = state.store_solutions(&request, solutions);

    let response = dto::SolveResponse {
        solutions: stored
            .iter()
            .map(|(solution_id, solution)| dto::Solution {
                solution_id: *solution_id,
                // TODO: always zero, the autopilot recomputes scores itself.
                score: 0,
                // The settlement signer, not the engine's declared identity:
                // the autopilot matches this against the on-chain signer.
                solver: state.solver_identity(),
                orders: solution
                    .trades
                    .iter()
                    .map(|trade| {
                        let order = request
                            .orders
                            .iter()
                            .find(|order| order.uid == trade.order_uid);
                        (
                            trade.order_uid,
                            dto::TradedAmounts {
                                executed_sell: trade.executed_amount,
                                // TODO: the engine wire carries one executed
                                // amount, the buy side reports the order's limit.
                                executed_buy: order.map(|o| o.buy_amount).unwrap_or_default(),
                            },
                        )
                    })
                    .collect::<HashMap<_, _>>(),
            })
            .collect(),
    };
    Ok(Json(response))
}
