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

#[derive(Clone, Debug, Default)]
pub struct Competition {
    pub auction_id: AuctionId,
    pub scores: Scores,
    pub participants: HashSet<H160>,  // solver addresses
    pub prices: BTreeMap<H160, U256>, // external prices for auction
}

#[derive(Clone, Debug, Default)]
pub struct Scores {
    pub winner: H160,
    pub winning_score: U256,
    pub reference_score: U256,
    pub block_deadline: u64,
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
                winner: ByteArray(competition.scores.winner.0),
                winning_score: u256_to_big_decimal(&competition.scores.winning_score),
                reference_score: u256_to_big_decimal(&competition.scores.reference_score),
                block_deadline: competition
                    .scores
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
