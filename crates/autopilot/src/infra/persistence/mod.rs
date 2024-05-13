use {
    crate::{
        boundary,
        database::{order_events::store_order_events, Postgres},
        domain::{self, eth},
    },
    anyhow::Context,
    chrono::Utc,
    number::conversions::big_decimal_to_u256,
    primitive_types::{H160, H256},
    std::{collections::HashMap, sync::Arc},
    tracing::Instrument,
};

pub mod cli;
pub mod dto;

#[derive(Clone)]
pub struct Persistence {
    s3: Option<s3::Uploader>,
    postgres: Arc<Postgres>,
}

impl Persistence {
    pub async fn new(config: Option<s3::Config>, postgres: Arc<Postgres>) -> Self {
        Self {
            s3: match config {
                Some(config) => Some(s3::Uploader::new(config).await),
                None => None,
            },
            postgres,
        }
    }

    /// There is always only one `current` auction.
    ///
    /// This method replaces the current auction with the given one.
    ///
    /// If the given auction is successfully saved, it is also archived.
    pub async fn replace_current_auction(
        &self,
        auction: &domain::Auction,
    ) -> Result<domain::auction::Id, Error> {
        let auction = dto::auction::from_domain(auction.clone());
        self.postgres
            .replace_current_auction(&auction)
            .await
            .map(|auction_id| {
                self.archive_auction(auction_id, auction);
                auction_id
            })
            .map_err(Error::DbError)
    }

    pub async fn solvable_orders(
        &self,
        min_valid_to: u32,
    ) -> Result<boundary::SolvableOrders, Error> {
        self.postgres
            .solvable_orders(min_valid_to)
            .await
            .map_err(Error::DbError)
    }

    /// Saves the given auction to storage for debugging purposes.
    ///
    /// There is no intention to retrieve this data programmatically.
    fn archive_auction(&self, id: domain::auction::Id, instance: dto::auction::Auction) {
        let Some(uploader) = self.s3.clone() else {
            return;
        };
        tokio::spawn(
            async move {
                match uploader.upload(id.to_string(), &instance).await {
                    Ok(key) => {
                        tracing::info!(?key, "uploaded auction to s3");
                    }
                    Err(err) => {
                        tracing::warn!(?err, "failed to upload auction to s3");
                    }
                }
            }
            .instrument(tracing::Span::current()),
        );
    }

    /// Saves the competition data to the DB
    pub async fn save_competition(&self, competition: &boundary::Competition) -> Result<(), Error> {
        self.postgres
            .save_competition(competition)
            .await
            .map_err(Error::DbError)
    }

    /// Inserts an order event for each order uid in the given set.
    /// Unique order uids are required to avoid inserting events with the same
    /// label within the same order_uid. If this function encounters an error it
    /// will only be printed. More elaborate error handling is not necessary
    /// because this is just debugging information.
    pub fn store_order_events(
        &self,
        order_uids: Vec<domain::OrderUid>,
        label: boundary::OrderEventLabel,
    ) {
        let db = self.postgres.clone();
        tokio::spawn(
            async move {
                let mut tx = db.pool.acquire().await.expect("failed to acquire tx");
                store_order_events(&mut tx, order_uids, label, Utc::now()).await;
            }
            .instrument(tracing::Span::current()),
        );
    }

    /// Saves the given fee policies to the DB as a single batch.
    pub async fn store_fee_policies(
        &self,
        auction_id: domain::auction::Id,
        fee_policies: Vec<(domain::OrderUid, Vec<domain::fee::Policy>)>,
    ) -> anyhow::Result<()> {
        let mut ex = self.postgres.pool.begin().await.context("begin")?;
        for chunk in fee_policies.chunks(self.postgres.config.insert_batch_size.get()) {
            crate::database::fee_policies::insert_batch(&mut ex, auction_id, chunk.iter().cloned())
                .await
                .context("fee_policies::insert_batch")?;
        }

        ex.commit().await.context("commit")
    }

    /// Retrieves the transaction hash for the settlement with the given
    /// auction_id.
    pub async fn find_tx_hash_by_auction_id(&self, auction_id: i64) -> Result<Option<H256>, Error> {
        self.postgres
            .find_tx_hash_by_auction_id(auction_id)
            .await
            .map_err(Error::DbError)
    }

    /// Get native token prices.
    pub async fn auction_prices(
        &self,
        auction: domain::auction::Id,
    ) -> Result<HashMap<eth::TokenAddress, domain::auction::Price>, Error> {
        let mut ex = self
            .postgres
            .pool
            .begin()
            .await
            .context("begin")
            .map_err(Error::DbError)?;

        let db_prices = database::auction_prices::fetch(&mut ex, auction)
            .await
            .context("fetch")
            .map_err(Error::DbError)?;

        let mut prices = HashMap::new();
        for price in db_prices {
            let token = eth::H160(price.token.0).into();
            let price = big_decimal_to_u256(&price.price)
                .ok_or(domain::auction::InvalidPrice)
                .and_then(|p| domain::auction::Price::new(p.into()))
                .map_err(AuctionError::Price)?;
            prices.insert(token, price);
        }

        Ok(prices)
    }

    /// Get auction data related to the given settlement.
    pub async fn get_settlement_auction(
        &self,
        settlement: &domain::settlement::Settlement,
    ) -> Result<domain::settlement::auction2::Auction, Error> {
        let mut ex = self
            .postgres
            .pool
            .begin()
            .await
            .context("begin")
            .map_err(Error::DbError)?;

        let auction = settlement.auction_id();

        // auction prices
        let db_prices = database::auction_prices::fetch(&mut ex, auction)
            .await
            .context("fetch auction prices")
            .map_err(Error::DbError)?;

        let mut prices = HashMap::new();
        for price in db_prices {
            let token = eth::H160(price.token.0).into();
            let price = big_decimal_to_u256(&price.price)
                .ok_or(domain::auction::InvalidPrice)
                .and_then(|p| domain::auction::Price::new(p.into()))
                .map_err(AuctionError::Price)?;
            prices.insert(token, price);
        }

        // scores
        let scores = database::settlement_scores::fetch(&mut ex, auction)
            .await
            .context("fetch scores")?
            .ok_or(AuctionError::MissingScore)?;

        // promised calldata
        let calldata = database::settlement_call_data::fetch(&mut ex, auction)
            .await
            .context("fetch call data")?
            .ok_or(AuctionError::MissingCalldata)?;

        let settled_orders = settlement.order_uids();
        //let missing_orders =

        Ok(domain::settlement::auction2::Auction {
            settlement: settlement.clone(),
            prices,
            winner: H160(scores.winner.0).into(),
            winner_score: big_decimal_to_u256(&scores.winning_score).unwrap(),
            winner_calldata: todo!(),
            deadline: todo!(),
            missing_orders: todo!(),
            fee_policies: todo!(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read data from database")]
    DbError(#[from] anyhow::Error),
    #[error(transparent)]
    Auction(#[from] AuctionError),
}

#[derive(Debug, thiserror::Error)]
pub enum AuctionError {
    #[error(transparent)]
    Price(#[from] domain::auction::InvalidPrice),
    #[error("score not found in the database")]
    MissingScore,
    #[error("calldata not found in the database")]
    MissingCalldata,
}
