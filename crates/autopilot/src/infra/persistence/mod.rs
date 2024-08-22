use {
    crate::{
        boundary,
        database::{order_events::store_order_events, Postgres},
        domain::{self, competition, eth},
        infra::persistence::dto::AuctionId,
    },
    anyhow::Context,
    boundary::database::byte_array::ByteArray,
    chrono::{DateTime, Utc},
    futures::TryStreamExt,
    number::conversions::big_decimal_to_u256,
    primitive_types::{H160, H256},
    std::{
        collections::{HashMap, HashSet},
        sync::Arc,
    },
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

    pub async fn read_quotes(
        &self,
        orders: impl Iterator<Item = &domain::OrderUid>,
    ) -> anyhow::Result<HashMap<domain::OrderUid, domain::Quote>> {
        Ok(self.postgres.read_quotes(orders).await?)
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

    /// Saves the surplus capturing jit order owners to the DB
    pub async fn save_surplus_capturing_jit_orders_orders(
        &self,
        auction_id: AuctionId,
        surplus_capturing_jit_order_owners: &[domain::eth::Address],
    ) -> Result<(), Error> {
        self.postgres
            .save_surplus_capturing_jit_orders_orders(
                auction_id,
                &surplus_capturing_jit_order_owners
                    .iter()
                    .map(|address| ByteArray(address.0.into()))
                    .collect::<Vec<_>>(),
            )
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

    /// Checks if an auction already has an accociated settlement.
    ///
    /// This function is used to detect processing of a staging settlement on
    /// production and vice versa, because staging and production environments
    /// don't have a disjunctive sets of auction ids.
    pub async fn auction_has_settlement(
        &self,
        auction_id: domain::auction::Id,
    ) -> Result<bool, DatabaseError> {
        let _timer = Metrics::get()
            .database_queries
            .with_label_values(&["auction_has_settlement"])
            .start_timer();

        let mut ex = self.postgres.pool.begin().await?;
        Ok(database::settlements::already_processed(&mut ex, auction_id).await?)
    }

    /// Get auction data.
    pub async fn get_auction(
        &self,
        auction_id: domain::auction::Id,
    ) -> Result<domain::settlement::Auction, error::Auction> {
        let _timer = Metrics::get()
            .database_queries
            .with_label_values(&["get_auction"])
            .start_timer();

        let mut ex = self
            .postgres
            .pool
            .begin()
            .await
            .map_err(error::Auction::BadCommunication)?;

        let surplus_capturing_jit_order_owners =
            database::surplus_capturing_jit_order_owners::fetch(&mut ex, auction_id)
                .await
                .map_err(error::Auction::BadCommunication)?
                .ok_or(error::Auction::NotFound)?
                .into_iter()
                .map(|owner| eth::H160(owner.0).into())
                .collect();

        let prices = database::auction_prices::fetch(&mut ex, auction_id)
            .await
            .map_err(error::Auction::BadCommunication)?
            .into_iter()
            .map(|price| {
                let token = eth::H160(price.token.0).into();
                let price = big_decimal_to_u256(&price.price)
                    .ok_or(domain::auction::InvalidPrice)
                    .and_then(|p| domain::auction::Price::new(p.into()))
                    .map_err(|_err| error::Auction::InvalidPrice(token));
                price.map(|price| (token, price))
            })
            .collect::<Result<_, _>>()?;

        let orders = {
            // get all orders from a competition auction
            let auction_orders = database::auction_orders::fetch(&mut ex, auction_id)
                .await
                .map_err(error::Auction::BadCommunication)?
                .ok_or(error::Auction::NotFound)?
                .into_iter()
                .map(|order| domain::OrderUid(order.0))
                .collect::<HashSet<_>>();

            // get fee policies for all orders that were part of the competition auction
            let fee_policies = database::fee_policies::fetch_all(
                &mut ex,
                auction_orders
                    .iter()
                    .map(|o| (auction_id, ByteArray(o.0)))
                    .collect::<Vec<_>>()
                    .as_slice(),
            )
            .await
            .map_err(error::Auction::BadCommunication)?
            .into_iter()
            .map(|((_, order), policies)| (domain::OrderUid(order.0), policies))
            .collect::<HashMap<_, _>>();

            // get quotes for all orders with PriceImprovement fee policy
            let quotes = self
                .postgres
                .read_quotes(fee_policies.iter().filter_map(|(order_uid, policies)| {
                    policies
                        .iter()
                        .any(|policy| {
                            matches!(
                                policy.kind,
                                database::fee_policies::FeePolicyKind::PriceImprovement
                            )
                        })
                        .then_some(order_uid)
                }))
                .await
                .map_err(error::Auction::BadCommunication)?;

            // compile order data
            let mut orders = HashMap::new();
            for order in auction_orders.iter() {
                let order_policies = match fee_policies.get(order) {
                    Some(policies) => policies
                        .iter()
                        .cloned()
                        .map(|policy| {
                            dto::fee_policy::try_into_domain(policy, quotes.get(order))
                                .map_err(|err| error::Auction::InvalidFeePolicy(err, *order))
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                    None => vec![],
                };
                orders.insert(*order, order_policies);
            }
            orders
        };

        Ok(domain::settlement::Auction {
            id: auction_id,
            orders,
            prices,
            surplus_capturing_jit_order_owners,
        })
    }

    /// Returns the proposed solver solution that won the competition for a
    /// given auction.
    ///
    /// It is expected for a solution to exist, so missing data is considered an
    /// error.
    ///
    /// Returns error for old non-colocated auctions.
    pub async fn get_winning_solution(
        &self,
        auction_id: domain::auction::Id,
    ) -> Result<domain::competition::Solution, error::Solution> {
        let _timer = Metrics::get()
            .database_queries
            .with_label_values(&["get_competition_winner"])
            .start_timer();

        let mut ex = self
            .postgres
            .pool
            .begin()
            .await
            .map_err(error::Solution::BadCommunication)?;

        let competition = database::settlement_scores::fetch(&mut ex, auction_id)
            .await
            .map_err(error::Solution::BadCommunication)?
            .ok_or(error::Solution::NotFound)?;

        let winner = H160(competition.winner.0).into();
        let score = competition::Score::new(
            big_decimal_to_u256(&competition.winning_score)
                .ok_or(error::Solution::InvalidScore(anyhow::anyhow!(
                    "database score"
                )))?
                .into(),
        )
        .map_err(|err| error::Solution::InvalidScore(anyhow::anyhow!("score, {}", err)))?;

        let solution = {
            // TODO: stabilize the solver competition table to get promised solution.
            let solver_competition = database::solver_competition::load_by_id(&mut ex, auction_id)
                .await
                .map_err(error::Solution::BadCommunication)?
                .ok_or(error::Solution::NotFound)?;
            let competition: model::solver_competition::SolverCompetitionDB =
                serde_json::from_value(solver_competition.json)
                    .context("deserialize SolverCompetitionDB")
                    .map_err(error::Solution::InvalidSolverCompetition)?;
            let winning_solution = competition
                .solutions
                .last()
                .ok_or(error::Solution::NotFound)?;
            let mut orders = HashMap::new();
            for order in winning_solution.orders.iter() {
                match order {
                    model::solver_competition::Order::Colocated {
                        id,
                        sell_amount,
                        buy_amount,
                    } => {
                        orders.insert(
                            domain::OrderUid(id.0),
                            competition::TradedAmounts {
                                sell: (*sell_amount).into(),
                                buy: (*buy_amount).into(),
                            },
                        );
                    }
                    model::solver_competition::Order::Legacy {
                        id: _,
                        executed_amount: _,
                    } => return Err(error::Solution::NotFound),
                }
            }
            let mut prices = HashMap::new();
            for (token, price) in winning_solution.clearing_prices.clone().into_iter() {
                prices.insert(
                    token.into(),
                    domain::auction::Price::new(price.into())
                        .map_err(|_| error::Solution::InvalidPrice(eth::TokenAddress(token)))?,
                );
            }
            competition::Solution::new(winner, score, orders, prices)
        };

        Ok(solution)
    }

    pub async fn orders_after(
        &self,
        after_timestamp: DateTime<Utc>,
        min_valid_to: i64,
    ) -> anyhow::Result<Vec<database::orders::ExtendedOrder>> {
        let _timer = Metrics::get()
            .database_queries
            .with_label_values(&["orders_after"])
            .start_timer();
        let mut ex = self.postgres.pool.begin().await.context("begin")?;
        Ok(
            database::orders::solvable_orders(&mut ex, after_timestamp, min_valid_to)
                .try_collect()
                .await?,
        )
    }

    pub async fn trades_after(
        &self,
        after_block: i64,
    ) -> anyhow::Result<HashMap<domain::OrderUid, database::trades::TradedAmounts>> {
        let _timer = Metrics::get()
            .database_queries
            .with_label_values(&["trades_after"])
            .start_timer();
        let mut ex = self.postgres.pool.begin().await.context("begin")?;
        Ok(database::trades::trades_after(&mut ex, after_block)
            .map_ok(|trade| (domain::OrderUid(trade.order_uid.0), trade))
            .try_collect()
            .await?)
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
struct Metrics {
    /// Timing of db queries.
    #[metric(name = "persistence_database_queries", labels("type"))]
    database_queries: prometheus::HistogramVec,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed communication with the database")]
    DbError(#[from] anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
#[error("failed communication with the database")]
pub struct DatabaseError(#[from] pub sqlx::Error);

pub mod error {
    use super::*;

    #[derive(Debug, thiserror::Error)]
    pub enum Auction {
        #[error("failed communication with the database: {0}")]
        BadCommunication(#[from] sqlx::Error),
        #[error("auction not found")]
        NotFound,
        #[error("invalid fee policy fetched from database: {0} for order: {1}")]
        InvalidFeePolicy(dto::fee_policy::Error, domain::OrderUid),
        #[error("invalid price fetched from database for token: {0:?}")]
        InvalidPrice(eth::TokenAddress),
    }

    #[derive(Debug, thiserror::Error)]
    pub enum Solution {
        #[error("failed communication with the database: {0}")]
        BadCommunication(#[from] sqlx::Error),
        #[error("solution not found")]
        NotFound,
        #[error("invalid score fetched from database: {0}")]
        InvalidScore(anyhow::Error),
        #[error("invalid price fetched from database for token: {0:?}")]
        InvalidPrice(eth::TokenAddress),
        #[error("invalid solver competition data fetched from database: {0}")]
        InvalidSolverCompetition(anyhow::Error),
    }
}
