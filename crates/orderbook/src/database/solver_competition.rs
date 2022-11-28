use super::Postgres;
use crate::solver_competition::{Identifier, LoadSolverCompetitionError, SolverCompetitionStoring};
use anyhow::{Context, Result};
use database::byte_array::ByteArray;
use model::solver_competition::{SolverCompetitionAPI, SolverCompetitionDB};
use number_conversions::u256_to_big_decimal;
use primitive_types::H256;

#[async_trait::async_trait]
impl SolverCompetitionStoring for Postgres {
    async fn handle_request(&self, request: model::solver_competition::Request) -> Result<()> {
        let json = &serde_json::to_value(&request.competition)?;

        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["handle_solver_competition_request"])
            .start_timer();

        let mut ex = self.pool.begin().await.context("begin")?;

        database::solver_competition::save(&mut ex, request.auction, json)
            .await
            .context("solver_competition::save")?;

        let transaction = request.transaction;
        database::auction_transaction::upsert_auction_transaction(
            &mut ex,
            request.auction,
            &ByteArray(transaction.account.0),
            transaction.nonce.try_into().context("convert nonce")?,
        )
        .await
        .context("upsert_auction_transaction")?;

        for (order, execution) in request.executions {
            let surplus_fee = execution.surplus_fee.as_ref().map(u256_to_big_decimal);
            database::order_execution::save(
                &mut ex,
                &ByteArray(order.0),
                request.auction,
                execution.reward,
                surplus_fee.as_ref(),
            )
            .await
            .context("order_rewards::save")?;
        }

        ex.commit().await.context("commit")
    }

    async fn load_competition(
        &self,
        id: Identifier,
    ) -> Result<SolverCompetitionAPI, LoadSolverCompetitionError> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["load_solver_competition"])
            .start_timer();

        let mut ex = self.pool.acquire().await.map_err(anyhow::Error::from)?;
        match id {
            Identifier::Id(id) => database::solver_competition::load_by_id(&mut ex, id)
                .await
                .context("solver_competition::load_by_id")?
                .map(|row| (row.json, id, row.tx_hash.map(|hash| H256(hash.0)))),
            // TODO: change this query to use the auction_transaction and settlements tables to
            // find the tx hash.
            Identifier::Transaction(hash) => {
                database::solver_competition::load_by_tx_hash(&mut ex, &ByteArray(hash.0))
                    .await
                    .context("solver_competition::load_by_tx_hash")?
                    .map(|row| (row.json, row.id, Some(hash)))
            }
        }
        .map(
            |(json, auction_id, transaction_hash)| -> Result<_, LoadSolverCompetitionError> {
                let common: SolverCompetitionDB =
                    serde_json::from_value(json).context("deserialize SolverCompetitionDB")?;
                Ok(SolverCompetitionAPI {
                    auction_id,
                    transaction_hash,
                    common,
                })
            },
        )
        .ok_or(LoadSolverCompetitionError::NotFound)?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::solver_competition::{CompetitionAuction, SolverSettlement};
    use primitive_types::H160;

    #[tokio::test]
    #[ignore]
    async fn postgres_solver_competition_roundtrip() {
        let db = Postgres::new("postgresql://").unwrap();
        database::clear_DANGER(&db.pool).await.unwrap();

        let request = model::solver_competition::Request {
            auction: 0,
            transaction: model::solver_competition::Transaction {
                account: H160([7; 20]),
                nonce: 8,
            },
            competition: SolverCompetitionDB {
                gas_price: 1.,
                auction_start_block: 2,
                liquidity_collected_block: 3,
                competition_simulation_block: 4,
                auction: CompetitionAuction {
                    orders: vec![Default::default()],
                    prices: [Default::default()].into_iter().collect(),
                },
                solutions: vec![SolverSettlement {
                    solver: "asdf".to_string(),
                    objective: Default::default(),
                    clearing_prices: [Default::default()].into_iter().collect(),
                    orders: vec![Default::default()],
                    call_data: vec![1, 2],
                }],
            },
            executions: Default::default(),
        };
        db.handle_request(request.clone()).await.unwrap();
        let actual = db.load_competition(Identifier::Id(0)).await.unwrap();
        assert_eq!(actual.common, request.competition);
        assert_eq!(actual.auction_id, 0);
        assert_eq!(actual.transaction_hash, None);
    }

    #[tokio::test]
    #[ignore]
    async fn not_found_error() {
        let db = Postgres::new("postgresql://").unwrap();
        database::clear_DANGER(&db.pool).await.unwrap();

        let result = db
            .load_competition(Identifier::Transaction(Default::default()))
            .await
            .unwrap_err();
        assert!(matches!(result, LoadSolverCompetitionError::NotFound));
    }
}
