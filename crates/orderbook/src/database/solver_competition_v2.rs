use {
    super::Postgres,
    crate::solver_competition::LoadSolverCompetitionError,
    anyhow::{Context, Result},
    database::byte_array::ByteArray,
    model::{
        order::OrderUid,
        solver_competition_v2::{Auction, Order, Response, Solution},
    },
    number::conversions::big_decimal_to_u256,
    primitive_types::{H160, H256},
};

impl Postgres {
    pub async fn load_competition_by_id_v2(
        &self,
        auction_id: i64,
    ) -> Result<Response, LoadSolverCompetitionError> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["load_solver_competition_by_id_v2"])
            .start_timer();

        let mut ex = self.pool.acquire().await.map_err(anyhow::Error::from)?;
        database::solver_competition::load_by_id_v2(&mut ex, auction_id)
            .await
            .context("solver_competition::load_by_id")?
            .map(to_dto)
            .ok_or(LoadSolverCompetitionError::NotFound)?
    }

    pub async fn load_competition_by_tx_hash_v2(
        &self,
        tx_hash: H256,
    ) -> Result<Response, LoadSolverCompetitionError> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["load_solver_competition_by_tx_hash_v2"])
            .start_timer();

        let mut ex = self.pool.acquire().await.map_err(anyhow::Error::from)?;
        database::solver_competition::load_by_tx_hash_v2(&mut ex, ByteArray(tx_hash.0))
            .await
            .context("solver_competition::load_by_tx_hash")?
            .map(to_dto)
            .ok_or(LoadSolverCompetitionError::NotFound)?
    }

    pub async fn load_latest_competition_v2(&self) -> Result<Response, LoadSolverCompetitionError> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["load_latest_solver_competition_v2"])
            .start_timer();

        let mut ex = self.pool.acquire().await.map_err(anyhow::Error::from)?;
        database::solver_competition::load_latest_v2(&mut ex)
            .await
            .context("solver_competition::load_latest")?
            .map(to_dto)
            .ok_or(LoadSolverCompetitionError::NotFound)?
    }
}

fn to_dto(
    value: database::solver_competition::Response,
) -> Result<Response, LoadSolverCompetitionError> {
    Ok(Response {
        auction_id: value.auction_id,
        auction_start_block: value.auction_start_block,
        transaction_hash: value
            .transaction_hashes
            .into_iter()
            .map(|tx| H256(tx.0))
            .collect(),
        reference_scores: value
            .reference_scores
            .into_iter()
            .map(|(solver, score)| {
                Ok((
                    H160(solver.0),
                    big_decimal_to_u256(&score)
                        .context("could not convert reference score to U256")?,
                ))
            })
            .collect::<Result<_>>()?,
        auction: Auction {
            prices: value
                .auction
                .prices
                .into_iter()
                .map(|(token, price)| {
                    Ok((
                        H160(token.0),
                        big_decimal_to_u256(&price)
                            .context("could not convert native price to U256")?,
                    ))
                })
                .collect::<Result<_>>()?,
            orders: value
                .auction
                .orders
                .into_iter()
                .map(|o| OrderUid(o.0))
                .collect(),
        },
        solutions: value
            .solutions
            .into_iter()
            .map(|s| {
                Ok(Solution {
                    solver_address: H160(s.solver_address.0),
                    score: big_decimal_to_u256(&s.score)
                        .context("could not convert score to U256")?,
                    ranking: s.ranking as usize,
                    clearing_prices: s
                        .clearing_prices
                        .into_iter()
                        .map(|(token, price)| {
                            Ok((
                                H160(token.0),
                                big_decimal_to_u256(&price)
                                    .context("could not convert clearing price to U256")?,
                            ))
                        })
                        .collect::<Result<_>>()?,
                    orders: s
                        .orders
                        .into_iter()
                        .map(|o| {
                            Ok(Order {
                                id: OrderUid(o.id.0),
                                sell_amount: big_decimal_to_u256(&o.sell_amount)
                                    .context("could not convert sell amount to U256")?,
                                buy_amount: big_decimal_to_u256(&o.buy_amount)
                                    .context("could not convert buy amount to U256")?,
                            })
                        })
                        .collect::<Result<_>>()?,
                    is_winner: s.is_winner,
                    filtered_out: s.filtered_out,
                    tx_hash: s.tx_hash.map(|tx| H256(tx.0)),
                    reference_score: s
                        .reference_score
                        .map(|s| {
                            big_decimal_to_u256(&s)
                                .context("could not convert reference score to U256")
                        })
                        .transpose()?,
                })
            })
            .collect::<Result<_>>()?,
    })
}
