use {
    super::Postgres,
    crate::solver_competition::{Identifier, LoadSolverCompetitionError, SolverCompetitionStoring},
    anyhow::{Context, Result},
    database::{
        auction_participants::Participant,
        auction_prices::AuctionPrice,
        byte_array::ByteArray,
        settlement_scores::Score,
    },
    model::solver_competition::{SolverCompetitionAPI, SolverCompetitionDB},
    number::conversions::u256_to_big_decimal,
    primitive_types::H256,
};

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
                surplus_fee.as_ref(),
                Some(&u256_to_big_decimal(&execution.solver_fee)),
            )
            .await
            .context("order_execution::save")?;
        }

        database::settlement_scores::insert(
            &mut ex,
            Score {
                auction_id: request.auction,
                winner: ByteArray(request.scores.winner.0),
                winning_score: u256_to_big_decimal(&request.scores.winning_score),
                reference_score: u256_to_big_decimal(&request.scores.reference_score),
                block_deadline: request
                    .scores
                    .block_deadline
                    .try_into()
                    .context("convert block deadline")?,
                simulation_block: request
                    .competition
                    .competition_simulation_block
                    .try_into()
                    .context("convert simulation block")?,
            },
        )
        .await
        .context("settlement_scores::insert")?;

        database::auction_participants::insert(
            &mut ex,
            request
                .participants
                .iter()
                .map(|p| Participant {
                    auction_id: request.auction,
                    participant: ByteArray(p.0),
                })
                .collect::<Vec<_>>()
                .as_slice(),
        )
        .await
        .context("auction_participants::insert")?;

        database::auction_prices::insert(
            &mut ex,
            request
                .prices
                .iter()
                .map(|(token, price)| AuctionPrice {
                    auction_id: request.auction,
                    token: ByteArray(token.0),
                    price: u256_to_big_decimal(price),
                })
                .collect::<Vec<_>>()
                .as_slice(),
        )
        .await
        .context("auction_prices::insert")?;

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
    use {
        super::*,
        model::solver_competition::{CompetitionAuction, Scores, SolverSettlement},
        primitive_types::H160,
        std::collections::BTreeMap,
    };

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
                    solver_address: H160([1; 20]),
                    objective: Default::default(),
                    score: Default::default(),
                    ranking: Some(1),
                    clearing_prices: [Default::default()].into_iter().collect(),
                    orders: vec![],
                    call_data: vec![1, 2],
                    uninternalized_call_data: Some(vec![1, 2, 3, 4]),
                }],
            },
            executions: Default::default(),
            scores: Scores {
                winner: H160([1; 20]),
                winning_score: 100.into(),
                reference_score: 99.into(),
                block_deadline: 10,
            },
            participants: [H160([1; 20])].into(),
            prices: BTreeMap::from([(H160([1; 20]), 1.into())]),
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
