use {
    crate::domain::competition::Score,
    alloy::primitives::Address,
    anyhow::Context,
    database::{auction::AuctionId, byte_array::ByteArray},
    number::conversions::u256_to_big_decimal,
    std::collections::HashMap,
};

impl super::Postgres {
    pub async fn save_reference_scores(
        &self,
        auction_id: AuctionId,
        reference_scores: HashMap<Address, Score>,
    ) -> anyhow::Result<()> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["save_reference_scores"])
            .start_timer();

        let mut ex = self.pool.acquire().await.context("save_reference_scores")?;

        let reference_scores: Vec<_> = reference_scores
            .into_iter()
            .map(|(solver, score)| database::reference_scores::Score {
                auction_id,
                solver: ByteArray(solver.0.0),
                reference_score: u256_to_big_decimal(&score.get().0),
            })
            .collect();

        database::reference_scores::insert(&mut ex, &reference_scores)
            .await
            .context("reference_scores::insert")
    }
}
