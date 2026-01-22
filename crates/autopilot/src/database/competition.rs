use {
    crate::domain::competition::Score,
    alloy::primitives::{Address, U256},
    anyhow::Context,
    database::{
        auction::AuctionId,
        auction_prices::AuctionPrice,
        byte_array::ByteArray,
        surplus_capturing_jit_order_owners,
    },
    derive_more::Debug,
    model::solver_competition::SolverCompetitionDB,
    number::conversions::u256_to_big_decimal,
    std::collections::{BTreeMap, HashMap, HashSet},
};

#[derive(Clone, Default, Debug)]
pub struct Competition {
    pub auction_id: AuctionId,
    pub reference_scores: HashMap<Address, Score>,
    /// Addresses to which the CIP20 participation rewards will be payed out.
    /// Usually the same as the solver addresses.
    pub participants: HashSet<Address>,
    /// External prices for auction.
    pub prices: BTreeMap<Address, U256>,
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

        // offload CPU intensive work of serializing to a blocking thread so we can
        // already start with the DB queries in the mean time.
        let json = tokio::task::spawn_blocking(move || {
            serde_json::to_string(&competition.competition_table)
        });

        let mut ex = self.pool.begin().await.context("begin")?;

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
            .context("reference_scores::insert")?;

        database::auction_prices::insert(
            &mut ex,
            competition
                .prices
                .into_iter()
                .map(|(token, price)| AuctionPrice {
                    auction_id: competition.auction_id,
                    token: ByteArray(token.0.0),
                    price: u256_to_big_decimal(&price),
                })
                .collect::<Vec<_>>()
                .as_slice(),
        )
        .await
        .context("auction_prices::insert")?;

        let json = json
            .await
            .context("failed to await blocking task")?
            .context("failed to serialize solver competition")?;
        database::solver_competition::save(&mut ex, competition.auction_id, &json)
            .await
            .context("solver_competition::save")?;

        ex.commit().await.context("commit")
    }

    /// Saves the surplus capturing jit order owners to the DB
    pub async fn save_surplus_capturing_jit_order_owners(
        &self,
        auction_id: AuctionId,
        surplus_capturing_jit_order_owners: &[database::Address],
    ) -> anyhow::Result<()> {
        let mut ex = self.pool.acquire().await.context("acquire")?;

        surplus_capturing_jit_order_owners::insert(
            &mut ex,
            auction_id,
            surplus_capturing_jit_order_owners,
        )
        .await?;

        Ok(())
    }
}
