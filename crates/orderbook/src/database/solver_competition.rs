use {
    super::Postgres,
    crate::solver_competition::{Identifier, LoadSolverCompetitionError, SolverCompetitionStoring},
    alloy::primitives::B256,
    anyhow::{Context, Result},
    database::byte_array::ByteArray,
    model::{
        AuctionId,
        solver_competition::{SolverCompetitionAPI, SolverCompetitionDB},
    },
    sqlx::types::JsonValue,
};

fn deserialize_solver_competition(
    json: JsonValue,
    auction_id: AuctionId,
    transaction_hashes: Vec<B256>,
) -> Result<SolverCompetitionAPI, LoadSolverCompetitionError> {
    let common: SolverCompetitionDB =
        serde_json::from_value(json).context("deserialize SolverCompetitionDB")?;
    Ok(SolverCompetitionAPI {
        auction_id,
        transaction_hashes,
        common,
    })
}

#[async_trait::async_trait]
impl SolverCompetitionStoring for Postgres {
    async fn load_competition(
        &self,
        id: Identifier,
        after_block: Option<i64>,
    ) -> Result<SolverCompetitionAPI, LoadSolverCompetitionError> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["load_solver_competition"])
            .start_timer();

        let mut ex = self.pool.acquire().await.map_err(anyhow::Error::from)?;
        let competition = match id {
            Identifier::Id(id) => database::solver_competition::load_by_id(&mut ex, id)
                .await
                .context("solver_competition::load_by_id")?
                .map(|row| {
                    deserialize_solver_competition(
                        row.json,
                        row.id,
                        row.tx_hashes.iter().map(|hash| B256::new(hash.0)).collect(),
                    )
                }),
            Identifier::Transaction(hash) => {
                database::solver_competition::load_by_tx_hash(&mut ex, &ByteArray(hash.0))
                    .await
                    .context("solver_competition::load_by_tx_hash")?
                    .map(|row| {
                        deserialize_solver_competition(
                            row.json,
                            row.id,
                            row.tx_hashes.iter().map(|hash| B256::new(hash.0)).collect(),
                        )
                    })
            }
        }
        .ok_or(LoadSolverCompetitionError::NotFound)??;
        hide_before_deadline(&mut ex, competition, after_block).await
    }

    async fn load_latest_competition(
        &self,
        after_block: Option<i64>,
    ) -> Result<SolverCompetitionAPI, LoadSolverCompetitionError> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["load_latest_solver_competition"])
            .start_timer();

        let mut ex = self.pool.acquire().await.map_err(anyhow::Error::from)?;
        let competition = database::solver_competition::load_latest_competition(&mut ex)
            .await
            .context("solver_competition::load_latest_competition")?
            .map(|row| {
                deserialize_solver_competition(
                    row.json,
                    row.id,
                    row.tx_hashes.iter().map(|hash| B256::new(hash.0)).collect(),
                )
            })
            .ok_or(LoadSolverCompetitionError::NotFound)??;
        hide_before_deadline(&mut ex, competition, after_block).await
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
        .context("solver_competition::load_latest_competitions")?
        .into_iter()
        .map(|row| {
            deserialize_solver_competition(
                row.json,
                row.id,
                row.tx_hashes.iter().map(|hash| B256::new(hash.0)).collect(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

        Ok(latest_competitions)
    }
}

/// V1 competitions don't store the deadline in the legacy JSON format, so we
/// check it via a separate query against `competition_auctions`.
async fn hide_before_deadline(
    ex: &mut sqlx::PgConnection,
    competition: SolverCompetitionAPI,
    after_block: Option<i64>,
) -> Result<SolverCompetitionAPI, LoadSolverCompetitionError> {
    if let Some(block) = after_block {
        let deadline: Option<i64> =
            sqlx::query_scalar("SELECT deadline FROM competition_auctions WHERE id = $1")
                .bind(competition.auction_id)
                .fetch_optional(&mut *ex)
                .await
                .map_err(|e| LoadSolverCompetitionError::Other(e.into()))?;
        if deadline.is_some_and(|d| d >= block) {
            return Err(LoadSolverCompetitionError::NotFound);
        }
    }
    Ok(competition)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn not_found_error() {
        let db = Postgres::try_new("postgresql://", Default::default()).unwrap();
        database::clear_DANGER(&db.pool).await.unwrap();

        let result = db
            .load_competition(Identifier::Transaction(Default::default()), None)
            .await
            .unwrap_err();
        assert!(matches!(result, LoadSolverCompetitionError::NotFound));
    }
}
