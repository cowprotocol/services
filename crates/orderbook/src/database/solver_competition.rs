use {
    super::Postgres,
    crate::solver_competition::{Identifier, LoadSolverCompetitionError, SolverCompetitionStoring},
    anyhow::{Context, Result},
    database::{byte_array::ByteArray, solver_competition::LoadCompetition},
    model::{
        auction::AuctionId,
        solver_competition::{SolverCompetitionAPI, SolverCompetitionDB},
    },
    number::conversions::big_decimal_to_u256,
    primitive_types::{H160, H256, U256},
    sqlx::types::JsonValue,
};

fn deserialize_solver_competition(
    json: JsonValue,
    auction_id: AuctionId,
    transaction_hashes: Vec<H256>,
    winner_txs: Vec<(H256, H160, U256)>,
) -> Result<SolverCompetitionAPI, LoadSolverCompetitionError> {
    let mut common: SolverCompetitionDB =
        serde_json::from_value(json).context("deserialize SolverCompetitionDB")?;

    for solution in &mut common.solutions {
        if !solution.is_winner {
            continue;
        }
        if let Some(score) = solution.score.as_ref() {
            let sc = score.score();
            if let Some((tx, _, _)) = winner_txs
                .iter()
                .find(|(_, solver, s)| solver == &solution.solver_address && *s == sc)
            {
                solution.tx_hash = Some(*tx);
            }
        }
    }

    Ok(SolverCompetitionAPI {
        auction_id,
        transaction_hashes,
        common,
    })
}

async fn load_and_deserialize_competition<'a>(
    ex: &mut sqlx::PgConnection,
    competition: LoadCompetition,
) -> Result<SolverCompetitionAPI, LoadSolverCompetitionError> {
    let settlement_txs = database::solver_competition::fetch_settlement_txs(ex, competition.id)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|t| {
            (
                H256(t.tx_hash.0),
                H160(t.solver.0),
                big_decimal_to_u256(&t.score).unwrap_or_default(),
            )
        })
        .collect();

    deserialize_solver_competition(
        competition.json,
        competition.id,
        competition
            .tx_hashes
            .iter()
            .map(|hash| H256(hash.0))
            .collect(),
        settlement_txs,
    )
}

#[async_trait::async_trait]
impl SolverCompetitionStoring for Postgres {
    async fn load_competition(
        &self,
        id: Identifier,
    ) -> Result<SolverCompetitionAPI, LoadSolverCompetitionError> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["load_solver_competition"])
            .start_timer();

        let mut ex = self.pool.acquire().await.map_err(anyhow::Error::from)?;
        let competition = match id {
            Identifier::Id(id) => database::solver_competition::load_by_id(&mut ex, id)
                .await
                .context("solver_competition::load_by_id")?,
            Identifier::Transaction(hash) => {
                database::solver_competition::load_by_tx_hash(&mut ex, &ByteArray(hash.0))
                    .await
                    .context("solver_competition::load_by_tx_hash")?
            }
        };

        let competition = competition.ok_or(LoadSolverCompetitionError::NotFound)?;
        load_and_deserialize_competition(&mut ex, competition).await
    }

    async fn load_latest_competition(
        &self,
    ) -> Result<SolverCompetitionAPI, LoadSolverCompetitionError> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["load_latest_solver_competition"])
            .start_timer();

        let mut ex = self.pool.acquire().await.map_err(anyhow::Error::from)?;
        let latest_competition = database::solver_competition::load_latest_competition(&mut ex)
            .await
            .context("solver_competition::load_latest_competition")?;

        let latest_competition = latest_competition.ok_or(LoadSolverCompetitionError::NotFound)?;
        load_and_deserialize_competition(&mut ex, latest_competition).await
    }

    async fn load_latest_competitions(
        &self,
        latest_competitions_count: u32,
    ) -> Result<Vec<SolverCompetitionAPI>, LoadSolverCompetitionError> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["load_latest_competitions"])
            .start_timer();

        let mut ex = self.pool.acquire().await.map_err(anyhow::Error::from)?;

        let latest_competitions = database::solver_competition::load_latest_competitions(
            &mut ex,
            latest_competitions_count,
        )
        .await
        .context("solver_competition::load_latest_competitions")?;

        let mut result = Vec::new();
        for competition in latest_competitions {
            result.push(load_and_deserialize_competition(&mut ex, competition).await?);
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn not_found_error() {
        let db = Postgres::try_new("postgresql://").unwrap();
        database::clear_DANGER(&db.pool).await.unwrap();

        let result = db
            .load_competition(Identifier::Transaction(Default::default()))
            .await
            .unwrap_err();
        assert!(matches!(result, LoadSolverCompetitionError::NotFound));
    }
}
