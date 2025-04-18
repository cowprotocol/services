use {
    anyhow::Context,
    database::{
        Address,
        auction::AuctionId,
        auction_participants::Participant,
        auction_prices::AuctionPrice,
        byte_array::ByteArray,
        reference_scores::Score,
        surplus_capturing_jit_order_owners,
    },
    derive_more::Debug,
    model::solver_competition::SolverCompetitionDB,
    number::conversions::u256_to_big_decimal,
    primitive_types::{H160, U256},
    std::collections::{BTreeMap, HashSet},
};

#[derive(Clone, Default, Debug)]
pub struct Competition {
    pub auction_id: AuctionId,
    pub reference_scores: Vec<ReferenceScore>,
    /// Addresses to which the CIP20 participation rewards will be payed out.
    /// Usually the same as the solver addresses.
    pub participants: HashSet<H160>,
    /// External prices for auction.
    pub prices: BTreeMap<H160, U256>,
    /// Winner receives performance rewards if a settlement is finalized on
    /// chain before this block height.
    pub block_deadline: u64,
    pub competition_simulation_block: u64,
    pub competition_table: SolverCompetitionDB,
}

#[derive(Clone, Default, Debug)]
pub struct ReferenceScore {
    pub solver: H160,
    pub reference_score: U256,
}

impl super::Postgres {
    pub async fn save_competition(&self, competition: &Competition) -> anyhow::Result<()> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["save_competition"])
            .start_timer();

        let json = &serde_json::to_value(&competition.competition_table)?;

        let mut ex = self.pool.begin().await.context("begin")?;

        database::solver_competition::save_solver_competition(
            &mut ex,
            competition.auction_id,
            json,
        )
        .await
        .context("solver_competition::save_solver_competition")?;

        let reference_scores = competition
            .reference_scores
            .iter()
            .map(|score| Score {
                auction_id: competition.auction_id,
                solver: ByteArray(score.solver.0),
                reference_score: u256_to_big_decimal(&score.reference_score),
            })
            .collect::<Vec<_>>();

        database::reference_scores::insert(&mut ex, &reference_scores)
            .await
            .context("reference_scores::insert")?;

        database::auction_participants::insert(
            &mut ex,
            competition
                .participants
                .iter()
                .map(|p| Participant {
                    auction_id: competition.auction_id,
                    participant: ByteArray(p.0),
                })
                .collect::<Vec<_>>()
                .as_slice(),
        )
        .await
        .context("auction_participants::insert")?;

        database::auction_prices::insert(
            &mut ex,
            competition
                .prices
                .iter()
                .map(|(token, price)| AuctionPrice {
                    auction_id: competition.auction_id,
                    token: ByteArray(token.0),
                    price: u256_to_big_decimal(price),
                })
                .collect::<Vec<_>>()
                .as_slice(),
        )
        .await
        .context("auction_prices::insert")?;

        database::auction_orders::insert(
            &mut ex,
            competition.auction_id,
            competition
                .competition_table
                .auction
                .orders
                .iter()
                .map(|order| ByteArray(order.0))
                .collect::<Vec<_>>()
                .as_slice(),
        )
        .await
        .context("auction_orders::insert")?;

        ex.commit().await.context("commit")
    }

    /// Saves the surplus capturing jit order owners to the DB
    pub async fn save_surplus_capturing_jit_order_owners(
        &self,
        auction_id: AuctionId,
        surplus_capturing_jit_order_owners: &[Address],
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
