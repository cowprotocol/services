use super::Postgres;
use crate::solver_competition::{LoadSolverCompetitionError, SolverCompetitionStoring};
use anyhow::{Context, Result};
use model::solver_competition::{SolverCompetition, SolverCompetitionId};
use sqlx::types::Json;

#[async_trait::async_trait]
impl SolverCompetitionStoring for Postgres {
    async fn save(&self, data: SolverCompetition) -> Result<SolverCompetitionId> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["save_solver_competition"])
            .start_timer();

        const QUERY: &str = r#"
            INSERT INTO solver_competitions (json)
            VALUES ($1)
            RETURNING id
        ;"#;

        let (id,) = sqlx::query_as(QUERY)
            .bind(Json(data))
            .fetch_one(&self.pool)
            .await
            .context("failed to insert solver competition")?;

        Ok(id)
    }

    async fn load(
        &self,
        id: SolverCompetitionId,
    ) -> Result<SolverCompetition, LoadSolverCompetitionError> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["load_solver_competition"])
            .start_timer();

        const QUERY: &str = r#"
            SELECT json
            FROM solver_competitions
            WHERE id = $1
        ;"#;

        let (solver_competition,): (Json<SolverCompetition>,) = sqlx::query_as(QUERY)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .context("failed to get solver competition by ID")?
            .ok_or(LoadSolverCompetitionError::NotFound(id))?;

        Ok(solver_competition.0)
    }

    async fn next_solver_competition(&self) -> Result<SolverCompetitionId> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["next_solver_competition"])
            .start_timer();

        // The sequence we created for the serial `id` column can be queried for
        // the next value it will produce [0]. Note that the exact semenatics of
        // the sequence's next value depend on its `is_called` flag [1].
        //
        // [0]: <https://www.postgresql.org/docs/current/sql-createsequence.html>
        // [1]: <https://www.postgresql.org/docs/14/functions-sequence.html>
        const QUERY: &str = r#"
            SELECT
                CASE
                    WHEN is_called THEN last_value + 1
                    ELSE last_value
                END AS next
            FROM solver_competitions_id_seq
        ;"#;

        let (id,): (SolverCompetitionId,) = sqlx::query_as(QUERY)
            .fetch_one(&self.pool)
            .await
            .context("failed to get next solver competition ID")?;

        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethcontract::H256;

    #[tokio::test]
    #[ignore]
    async fn postgres_save_and_load_solver_competition_by_id() {
        let db = Postgres::new("postgresql://").unwrap();
        database::clear_DANGER(&db.pool).await.unwrap();

        let model = SolverCompetition {
            gas_price: 1.,
            auction_start_block: 2,
            liquidity_collected_block: 3,
            competition_simulation_block: 4,
            transaction_hash: Some(H256([5; 32])),
            auction: Default::default(),
            solutions: Default::default(),
        };

        let id = db.save(model.clone()).await.unwrap();
        let loaded = db.load(id).await.unwrap();

        assert_eq!(model, loaded);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_solver_competition_id_sequence() {
        let db = Postgres::new("postgresql://").unwrap();
        database::clear_DANGER(&db.pool).await.unwrap();

        let next_id = db.next_solver_competition().await.unwrap();
        // still the same
        assert_eq!(db.next_solver_competition().await.unwrap(), next_id);

        let id = db.save(Default::default()).await.unwrap();
        assert_eq!(id, next_id);

        let next_id_ = db.next_solver_competition().await.unwrap();
        assert_eq!(next_id_, next_id + 1);
    }
}
