use crate::{boundary, infra};

impl infra::Persistence {
    /// Saves the competition data to the DB
    pub async fn save_competition(&self, competition: &boundary::Competition) -> Result<(), Error> {
        self.postgres
            .save_competition(competition)
            .await
            .map_err(Error::DbError)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to write data to database")]
    DbError(#[from] anyhow::Error),
}
