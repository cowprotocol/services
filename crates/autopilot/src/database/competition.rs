use {
    anyhow::Context,
    database::{
        auction::AuctionId,
        auction_participants::Participant,
        auction_prices::AuctionPrice,
        byte_array::ByteArray,
        settlement_call_data::SettlementCallData,
        settlement_scores::Score,
    },
    derivative::Derivative,
    model::{order::OrderUid, solver_competition::SolverCompetitionDB},
    number::conversions::u256_to_big_decimal,
    primitive_types::{H160, U256},
    std::collections::{BTreeMap, HashSet},
};

#[derive(Clone, Debug)]
pub enum ExecutedFee {
    /// Fee is calculated by the solver and known upfront (before the settlement
    /// is finalized).
    Solver(U256),
    /// Fee is unknown before the settlement is finalized and is calculated in
    /// the postprocessing. Currently only used for limit orders.
    Surplus,
}

impl ExecutedFee {
    pub fn fee(&self) -> Option<&U256> {
        match self {
            ExecutedFee::Solver(fee) => Some(fee),
            ExecutedFee::Surplus => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct OrderExecution {
    pub order_id: OrderUid,
    pub executed_fee: ExecutedFee,
}

#[derive(Clone, Default, Derivative)]
#[derivative(Debug)]
pub struct Competition {
    pub auction_id: AuctionId,
    pub winner: H160,
    pub winning_score: U256,
    pub reference_score: U256,
    /// Addresses to which the CIP20 participation rewards will be payed out.
    /// Usually the same as the solver addresses.
    pub participants: HashSet<H160>,
    /// External prices for auction.
    pub prices: BTreeMap<H160, U256>,
    /// Winner receives performance rewards if a settlement is finalized on
    /// chain before this block height.
    pub block_deadline: u64,
    pub order_executions: Vec<OrderExecution>,
    pub competition_simulation_block: u64,
    /// Winner settlement call data
    #[derivative(Debug(format_with = "shared::debug_bytes"))]
    pub call_data: Vec<u8>,
    /// Uninternalized winner settlement call data
    #[derivative(Debug(format_with = "shared::debug_bytes"))]
    pub uninternalized_call_data: Vec<u8>,
    pub competition_table: SolverCompetitionDB,
}

impl super::Postgres {
    pub async fn save_competition(&self, competition: &Competition) -> anyhow::Result<()> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["save_competition"])
            .start_timer();

        let json = &serde_json::to_value(&competition.competition_table)?;

        let mut ex = self.0.begin().await.context("begin")?;

        database::solver_competition::save(&mut ex, competition.auction_id, json)
            .await
            .context("solver_competition::save")?;

        for order_execution in &competition.order_executions {
            let solver_fee = order_execution
                .executed_fee
                .fee()
                .copied()
                .unwrap_or_default();
            database::order_execution::save(
                &mut ex,
                &ByteArray(order_execution.order_id.0),
                competition.auction_id,
                None,
                &u256_to_big_decimal(&solver_fee),
            )
            .await
            .context("order_execution::save")?;
        }

        database::settlement_scores::insert(
            &mut ex,
            Score {
                auction_id: competition.auction_id,
                winner: ByteArray(competition.winner.0),
                winning_score: u256_to_big_decimal(&competition.winning_score),
                reference_score: u256_to_big_decimal(&competition.reference_score),
                block_deadline: competition
                    .block_deadline
                    .try_into()
                    .context("convert block deadline")?,
                simulation_block: competition
                    .competition_simulation_block
                    .try_into()
                    .context("convert simulation block")?,
            },
        )
        .await
        .context("settlement_scores::insert")?;

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

        database::settlement_call_data::insert(
            &mut ex,
            SettlementCallData {
                auction_id: competition.auction_id,
                call_data: competition.call_data.clone(),
                uninternalized_call_data: competition.uninternalized_call_data.clone(),
            },
        )
        .await
        .context("settlement_call_data::insert")?;

        ex.commit().await.context("commit")
    }
}
