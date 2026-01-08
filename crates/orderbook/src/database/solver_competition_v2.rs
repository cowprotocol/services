use {
    super::Postgres,
    crate::solver_competition::LoadSolverCompetitionError,
    alloy::primitives::{Address, B256},
    anyhow::{Context, Result},
    database::{byte_array::ByteArray, solver_competition_v2::SolverCompetition as DbResponse},
    model::{
        order::OrderUid,
        solver_competition_v2::{Auction, Order, Response as ApiResponse, Solution},
    },
    number::conversions::big_decimal_to_u256,
    std::collections::{BTreeMap, HashMap},
};

impl Postgres {
    pub async fn load_competition_by_id(
        &self,
        auction_id: i64,
    ) -> Result<ApiResponse, LoadSolverCompetitionError> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["load_solver_competition_by_id_v2"])
            .start_timer();

        let mut ex = self.pool.acquire().await.map_err(anyhow::Error::from)?;
        database::solver_competition_v2::load_by_id(&mut ex, auction_id)
            .await
            .context("solver_competition_v2::load_by_id")?
            .map(try_into_dto)
            .ok_or(LoadSolverCompetitionError::NotFound)?
    }

    pub async fn load_competition_by_tx_hash(
        &self,
        tx_hash: B256,
    ) -> Result<ApiResponse, LoadSolverCompetitionError> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["load_solver_competition_by_tx_hash_v2"])
            .start_timer();

        let mut ex = self.pool.acquire().await.map_err(anyhow::Error::from)?;
        database::solver_competition_v2::load_by_tx_hash(&mut ex, ByteArray(tx_hash.0))
            .await
            .context("solver_competition_v2::load_by_tx_hash")?
            .map(try_into_dto)
            .ok_or(LoadSolverCompetitionError::NotFound)?
    }

    pub async fn load_latest_competition(&self) -> Result<ApiResponse, LoadSolverCompetitionError> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["load_latest_solver_competition_v2"])
            .start_timer();

        let mut ex = self.pool.acquire().await.map_err(anyhow::Error::from)?;
        database::solver_competition_v2::load_latest(&mut ex)
            .await
            .context("solver_competition_v2::load_latest")?
            .map(try_into_dto)
            .ok_or(LoadSolverCompetitionError::NotFound)?
    }
}

fn try_into_dto(value: DbResponse) -> Result<ApiResponse, LoadSolverCompetitionError> {
    let native_prices: BTreeMap<_, _> = value
        .auction
        .price_tokens
        .into_iter()
        .zip(value.auction.price_values)
        .map(|(token, price)| {
            Ok((
                Address::new(token.0),
                big_decimal_to_u256(&price).context("could not convert native price to U256")?,
            ))
        })
        .collect::<Result<_>>()?;

    let settlements: HashMap<_, _> = value
        .settlements
        .into_iter()
        .map(|row| (row.solution_uid, B256::new(row.tx_hash.0)))
        .collect();

    let reference_scores: BTreeMap<_, _> = value
        .reference_scores
        .into_iter()
        .map(|row| {
            Ok((
                Address::new(row.solver.0),
                big_decimal_to_u256(&row.reference_score)
                    .context("could not convert reference score to U256")?,
            ))
        })
        .collect::<Result<_>>()?;

    let mut trades: HashMap<i64, Vec<Order>> = {
        let mut grouped_trades = HashMap::<i64, Vec<Order>>::default();
        for trade in value.trades {
            grouped_trades
                .entry(trade.solution_uid)
                .or_default()
                .push(Order {
                    id: OrderUid(trade.order_uid.0),
                    sell_amount: big_decimal_to_u256(&trade.executed_sell)
                        .context("could not convert sell amount to U256")?,
                    buy_amount: big_decimal_to_u256(&trade.executed_buy)
                        .context("could not convert buy amount to U256")?,
                    sell_token: Address::new(trade.sell_token.0),
                    buy_token: Address::new(trade.buy_token.0),
                });
        }
        grouped_trades
    };

    let mut solutions: Vec<Solution> = value
        .solutions
        .into_iter()
        .map(|solution| {
            let clearing_prices: BTreeMap<_, _> = solution
                .price_tokens
                .into_iter()
                .zip(solution.price_values)
                .map(|(token, price)| {
                    Ok((
                        Address::new(token.0),
                        big_decimal_to_u256(&price)
                            .context("could not convert clearing price to U256")?,
                    ))
                })
                .collect::<Result<_>>()?;

            Ok(Solution {
                solver_address: Address::new(solution.solver.0),
                score: big_decimal_to_u256(&solution.score)
                    .context("could not convert score to U256")?,
                ranking: solution.ranking,
                clearing_prices,
                orders: trades.remove(&solution.uid).unwrap_or_default(),
                is_winner: solution.is_winner,
                filtered_out: solution.filtered_out,
                tx_hash: settlements.get(&solution.uid).cloned(),
                reference_score: reference_scores
                    .get(&Address::new(solution.solver.0))
                    .copied(),
            })
        })
        .collect::<Result<_>>()?;

    // sort from worst to best to stay consistent with the old endpoint
    solutions.sort_by_key(|s| std::cmp::Reverse(s.ranking));

    Ok(ApiResponse {
        auction_id: value.auction.id,
        auction_start_block: value.auction.block,
        auction_deadline_block: value.auction.deadline,
        transaction_hashes: settlements.values().cloned().collect(),
        reference_scores,
        auction: Auction {
            prices: native_prices,
            orders: value
                .auction
                .order_uids
                .into_iter()
                .map(|o| OrderUid(o.0))
                .collect(),
        },
        solutions,
    })
}
