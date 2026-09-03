use {
    crate::domain::competition::Score,
    alloy::primitives::Address,
    anyhow::Context,
    database::{auction::AuctionId, byte_array::ByteArray},
    derive_more::Debug,
    model::solver_competition::SolverCompetitionDB,
    number::conversions::u256_to_big_decimal,
    std::collections::{HashMap, HashSet},
};

#[derive(Clone, Default, Debug)]
pub struct Competition {
    pub auction_id: AuctionId,
    pub reference_scores: HashMap<Address, Score>,
    /// Addresses to which the CIP20 participation rewards will be payed out.
    /// Usually the same as the solver addresses.
    pub participants: HashSet<Address>,
    /// Winner receives performance rewards if a settlement is finalized on
    /// chain before this block height.
    pub block_deadline: u64,
    pub competition_simulation_block: u64,
    pub competition_table: SolverCompetitionDB,
}

impl super::Postgres {
    pub async fn save_competition(&self, competition: Competition) -> anyhow::Result<()> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["save_competition"])
            .start_timer();

        let mut ex = self.pool.acquire().await.context("save_competition")?;

        let reference_scores: Vec<_> = competition
            .reference_scores
            .into_iter()
            .map(|(solver, score)| database::reference_scores::Score {
                auction_id: competition.auction_id,
                solver: ByteArray(solver.0.0),
                reference_score: u256_to_big_decimal(&score.get().0),
            })
            .collect();

        database::reference_scores::insert(&mut ex, &reference_scores)
            .await
            .context("reference_scores::insert")
    }
}
