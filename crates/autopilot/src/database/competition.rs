use {
    anyhow::Context,
    database::{
        auction::AuctionId,
        auction_participants::Participant,
        auction_prices::AuctionPrice,
        byte_array::ByteArray,
        settlement_scores::Score,
    },
    model::order::OrderUid,
    number_conversions::u256_to_big_decimal,
    primitive_types::{H160, U256},
    std::collections::{BTreeMap, HashSet},
};

#[derive(Clone, Debug)]
pub enum ExecutedFee {
    Solver(U256),
    /// Optional because, for partially fillable limit orders, surplus fee is
    /// unknown until the transaction is mined.
    Surplus(Option<U256>),
}

#[derive(Clone, Debug)]
pub struct OrderExecution {
    pub order_id: OrderUid,
    pub executed_fee: ExecutedFee,
}

#[derive(Debug, Clone, Default)]
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
}

impl super::Postgres {
    pub async fn save_competition(&self, competition: &Competition) -> anyhow::Result<()> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["save_competition"])
            .start_timer();

        let mut ex = self.0.begin().await.context("begin")?;

        for order_execution in &competition.order_executions {
            let (solver_fee, surplus_fee) = match order_execution.executed_fee {
                ExecutedFee::Solver(solver_fee) => (solver_fee, None),
                ExecutedFee::Surplus(surplus_fee) => (Default::default(), surplus_fee),
            };
            let surplus_fee = surplus_fee.as_ref().map(u256_to_big_decimal);
            database::order_execution::save(
                &mut ex,
                &ByteArray(order_execution.order_id.0),
                competition.auction_id,
                surplus_fee.as_ref(),
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

        ex.commit().await.context("commit")
    }
}
