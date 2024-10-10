use {
    super::Postgres,
    crate::solver_competition::{Identifier, LoadSolverCompetitionError, SolverCompetitionStoring},
    anyhow::{Context, Result},
    database::byte_array::ByteArray,
    model::{
        auction::AuctionId,
        solver_competition::{SolverCompetitionAPI, SolverCompetitionDB},
    },
    primitive_types::H256,
    sqlx::types::JsonValue,
};

fn deserialize_solver_competition(
    json: JsonValue,
    auction_id: AuctionId,
    transaction_hashes: Vec<H256>,
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
                .map(|row| {
                    deserialize_solver_competition(
                        row.json,
                        row.id,
                        row.tx_hashes.iter().map(|hash| H256(hash.0)).collect(),
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
                            row.tx_hashes.iter().map(|hash| H256(hash.0)).collect(),
                        )
                    })
            }
        }
        .ok_or(LoadSolverCompetitionError::NotFound)?
    }

    async fn load_latest_competition(
        &self,
    ) -> Result<SolverCompetitionAPI, LoadSolverCompetitionError> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["load_latest_solver_competition"])
            .start_timer();

        let mut ex = self.pool.acquire().await.map_err(anyhow::Error::from)?;
        database::solver_competition::load_latest_competition(&mut ex)
            .await
            .context("solver_competition::load_latest")?
            .map(|row| {
                deserialize_solver_competition(
                    row.json,
                    row.id,
                    row.tx_hashes.iter().map(|hash| H256(hash.0)).collect(),
                )
            })
            .ok_or(LoadSolverCompetitionError::NotFound)?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
