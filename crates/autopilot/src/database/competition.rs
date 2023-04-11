use {
    anyhow::Context,
    database::{
        auction::AuctionId,
        auction_participants::Participant,
        auction_prices::AuctionPrice,
        byte_array::ByteArray,
        settlement_scores::Score,
    },
    number_conversions::u256_to_big_decimal,
    primitive_types::{H160, U256},
    std::collections::{BTreeMap, HashSet},
};

#[derive(Clone, Default)]
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
}

// Skipped `prices` as too long.
impl std::fmt::Debug for Competition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Competition")
            .field("auction_id", &self.auction_id)
            .field("winner", &self.winner)
            .field("winning_score", &self.winning_score)
            .field("reference_score", &self.reference_score)
            .field("participants", &self.participants)
            .field("block_deadline", &self.block_deadline)
            .finish()
    }
}

impl super::Postgres {
    pub async fn save_competition(&self, competition: &Competition) -> anyhow::Result<()> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["save_competition"])
            .start_timer();

        let mut ex = self.0.begin().await.context("begin")?;

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
