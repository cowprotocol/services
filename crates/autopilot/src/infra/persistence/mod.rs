use {
    crate::{
        boundary,
        database::{order_events::store_order_events, Postgres},
        domain::{self, competition, eth},
        infra::persistence::dto::AuctionId,
    },
    anyhow::Context,
    bigdecimal::ToPrimitive,
    boundary::database::byte_array::ByteArray,
    chrono::{DateTime, Utc},
    database::{
        order_events::OrderEventLabel,
        orders::{
            BuyTokenDestination as DbBuyTokenDestination,
            SellTokenSource as DbSellTokenSource,
            SigningScheme as DbSigningScheme,
        },
        settlement_observations::Observation,
    },
    domain::auction::order::{
        BuyTokenDestination as DomainBuyTokenDestination,
        SellTokenSource as DomainSellTokenSource,
        SigningScheme as DomainSigningScheme,
    },
    futures::{StreamExt, TryStreamExt},
    itertools::Itertools,
    number::conversions::{big_decimal_to_u256, u256_to_big_decimal, u256_to_big_uint},
    primitive_types::{H160, H256},
    shared::db_order_conversions::full_order_into_model_order,
    std::{
        collections::{HashMap, HashSet},
        ops::DerefMut,
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
    ) -> Result<domain::auction::Id, DatabaseError> {
        let auction = dto::auction::from_domain(auction.clone());
        self.postgres
            .replace_current_auction(&auction)
            .await
            .map(|auction_id| {
                self.archive_auction(auction_id, auction);
                auction_id
            })
            .map_err(DatabaseError)
    }

    /// Finds solvable orders based on the order's min validity period.
    pub async fn all_solvable_orders(
        &self,
        min_valid_to: u32,
    ) -> Result<boundary::SolvableOrders, DatabaseError> {
        self.postgres
            .all_solvable_orders(min_valid_to)
            .await
            .map_err(DatabaseError)
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
    pub async fn save_competition(
        &self,
        competition: &boundary::Competition,
    ) -> Result<(), DatabaseError> {
        self.postgres
            .save_competition(competition)
            .await
            .map_err(DatabaseError)
    }

    /// Saves the surplus capturing jit order owners to the DB
    pub async fn save_surplus_capturing_jit_orders_orders(
        &self,
        auction_id: AuctionId,
        surplus_capturing_jit_order_owners: &[domain::eth::Address],
    ) -> Result<(), DatabaseError> {
        self.postgres
            .save_surplus_capturing_jit_orders_orders(
                auction_id,
                &surplus_capturing_jit_order_owners
                    .iter()
                    .map(|address| ByteArray(address.0.into()))
                    .collect::<Vec<_>>(),
            )
            .await
            .map_err(DatabaseError)
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
    pub async fn find_tx_hash_by_auction_id(
        &self,
        auction_id: i64,
    ) -> Result<Option<H256>, DatabaseError> {
        self.postgres
            .find_tx_hash_by_auction_id(auction_id)
            .await
            .map_err(DatabaseError)
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
            .map_err(error::Auction::DatabaseError)?;

        let surplus_capturing_jit_order_owners =
            database::surplus_capturing_jit_order_owners::fetch(&mut ex, auction_id)
                .await
                .map_err(error::Auction::DatabaseError)?
                .ok_or(error::Auction::NotFound)?
                .into_iter()
                .map(|owner| eth::H160(owner.0).into())
                .collect();

        let prices = database::auction_prices::fetch(&mut ex, auction_id)
            .await
            .map_err(error::Auction::DatabaseError)?
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
                .map_err(error::Auction::DatabaseError)?
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
            .map_err(error::Auction::DatabaseError)?
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
                .map_err(error::Auction::DatabaseError)?;

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
            .map_err(error::Solution::DatabaseError)?;

        let competition = database::settlement_scores::fetch(&mut ex, auction_id)
            .await
            .map_err(error::Solution::DatabaseError)?
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
                .map_err(error::Solution::DatabaseError)?
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

    /// Computes solvable orders based on the latest observed block number,
    /// order creation timestamp, and minimum validity period.
    pub async fn solvable_orders_after(
        &self,
        current_orders: HashMap<domain::OrderUid, model::order::Order>,
        after_timestamp: DateTime<Utc>,
        after_block: u64,
        min_valid_to: u32,
    ) -> anyhow::Result<boundary::SolvableOrders> {
        let after_block = i64::try_from(after_block).context("block number value exceeds i64")?;
        let started_at = chrono::offset::Utc::now();
        let mut tx = self.postgres.pool.begin().await.context("begin")?;
        // Set the transaction isolation level to REPEATABLE READ
        // so all the SELECT queries below are executed in the same database snapshot
        // taken at the moment before the first query is executed.
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
            .execute(tx.deref_mut())
            .await?;

        // Fetch the orders that were updated after the given block and were created or
        // cancelled after the given timestamp.
        let next_orders: HashMap<domain::OrderUid, model::order::Order> = {
            let _timer = Metrics::get()
                .database_queries
                .with_label_values(&["open_orders_after"])
                .start_timer();

            database::orders::open_orders_after(&mut tx, after_block, after_timestamp)
                .map(|result| match result {
                    Ok(order) => full_order_into_model_order(order)
                        .map(|order| (domain::OrderUid(order.metadata.uid.0), order)),
                    Err(err) => Err(anyhow::Error::from(err)),
                })
                .try_collect()
                .await?
        };

        // Fetch quotes for new orders and also update them for the cached ones since
        // they could also be updated.
        let updated_quotes = {
            let _timer = Metrics::get()
                .database_queries
                .with_label_values(&["read_quotes"])
                .start_timer();

            let all_order_uids = next_orders
                .keys()
                .chain(current_orders.keys())
                .unique()
                .map(|uid| ByteArray(uid.0))
                .collect::<Vec<_>>();

            database::orders::read_quotes(&mut tx, &all_order_uids)
                .await?
                .into_iter()
                .filter_map(|quote| {
                    let order_uid = domain::OrderUid(quote.order_uid.0);
                    dto::quote::into_domain(quote)
                        .map_err(|err| {
                            tracing::warn!(?order_uid, ?err, "failed to convert quote from db")
                        })
                        .ok()
                        .map(|quote| (order_uid, quote))
                })
                .collect()
        };

        let latest_settlement_block = database::orders::latest_settlement_block(&mut tx)
            .await?
            .to_u64()
            .context("latest_settlement_block is not u64")?;

        Self::build_solvable_orders(
            current_orders,
            next_orders,
            updated_quotes,
            latest_settlement_block,
            min_valid_to,
            started_at,
        )
    }

    fn build_solvable_orders(
        mut current_orders: HashMap<domain::OrderUid, model::order::Order>,
        next_orders: HashMap<domain::OrderUid, model::order::Order>,
        mut next_quotes: HashMap<domain::OrderUid, domain::Quote>,
        latest_settlement_block: u64,
        min_valid_to: u32,
        started_at: chrono::DateTime<chrono::Utc>,
    ) -> anyhow::Result<boundary::SolvableOrders> {
        // Blindly insert all new orders into the cache.
        for (uid, order) in next_orders {
            current_orders.insert(uid, order);
        }

        // Filter out all the invalid orders.
        current_orders.retain(|_uid, order| {
            let expired = order.data.valid_to < min_valid_to
                || order
                    .metadata
                    .ethflow_data
                    .as_ref()
                    .is_some_and(|data| data.user_valid_to < i64::from(min_valid_to));

            let invalidated = order.metadata.invalidated;
            let onchain_error = order
                .metadata
                .onchain_order_data
                .as_ref()
                .is_some_and(|data| data.placement_error.is_some());
            let fulfilled = {
                match order.data.kind {
                    model::order::OrderKind::Sell => {
                        order.metadata.executed_sell_amount
                            >= u256_to_big_uint(&order.data.sell_amount)
                    }
                    model::order::OrderKind::Buy => {
                        order.metadata.executed_buy_amount
                            >= u256_to_big_uint(&order.data.buy_amount)
                    }
                }
            };

            !expired && !invalidated && !onchain_error && !fulfilled
        });

        // Keep only relevant quotes.
        next_quotes.retain(|uid, _quote| current_orders.contains_key(uid));

        Ok(boundary::SolvableOrders {
            orders: current_orders,
            quotes: next_quotes,
            latest_settlement_block,
            fetched_from_db: started_at,
        })
    }

    /// Returns the oldest settlement event for which the accociated auction is
    /// not yet populated in the database.
    pub async fn get_settlement_without_auction(
        &self,
    ) -> Result<Option<domain::eth::Event>, DatabaseError> {
        let _timer = Metrics::get()
            .database_queries
            .with_label_values(&["get_settlement_without_auction"])
            .start_timer();

        let mut ex = self.postgres.pool.acquire().await?;
        let event = database::settlements::get_settlement_without_auction(&mut ex)
            .await?
            .map(|event| {
                let event = domain::eth::Event {
                    block: u64::try_from(event.block_number)
                        .context("negative block")?
                        .into(),
                    log_index: u64::try_from(event.log_index).context("negative log index")?,
                    transaction: eth::TxId(H256(event.tx_hash.0)),
                };
                Ok::<_, DatabaseError>(event)
            })
            .transpose()?;
        Ok(event)
    }

    pub async fn save_settlement(
        &self,
        event: domain::eth::Event,
        auction_id: domain::auction::Id,
        settlement: Option<&domain::settlement::Settlement>,
    ) -> Result<(), DatabaseError> {
        let _timer = Metrics::get()
            .database_queries
            .with_label_values(&["save_settlement"])
            .start_timer();

        let mut ex = self.postgres.pool.begin().await?;

        let block_number = i64::try_from(event.block.0).context("block overflow")?;
        let log_index = i64::try_from(event.log_index).context("log index overflow")?;

        database::settlements::update_settlement_auction(
            &mut ex,
            block_number,
            log_index,
            auction_id,
        )
        .await?;

        if let Some(settlement) = settlement {
            let gas = settlement.gas();
            let gas_price = settlement.gas_price();
            let surplus = settlement.surplus_in_ether();
            let fee = settlement.fee_in_ether();
            let order_fees = settlement.order_fees();
            let jit_orders = settlement.jit_orders();

            tracing::debug!(
                ?auction_id,
                hash = ?event.transaction,
                ?gas,
                ?gas_price,
                ?surplus,
                ?fee,
                ?order_fees,
                ?jit_orders,
                "settlement update",
            );

            database::settlement_observations::upsert(
                &mut ex,
                Observation {
                    block_number,
                    log_index,
                    gas_used: u256_to_big_decimal(&gas.0),
                    effective_gas_price: u256_to_big_decimal(&gas_price.0 .0),
                    surplus: u256_to_big_decimal(&surplus.0),
                    fee: u256_to_big_decimal(&fee.0),
                },
            )
            .await?;

            store_order_events(
                &mut ex,
                order_fees.keys().cloned().collect(),
                OrderEventLabel::Traded,
                Utc::now(),
            )
            .await;

            for (order, executed_fee) in order_fees {
                database::order_execution::save(
                    &mut ex,
                    &ByteArray(order.0),
                    auction_id,
                    block_number,
                    &u256_to_big_decimal(
                        &executed_fee.map(|fee| fee.total()).unwrap_or_default().0,
                    ),
                )
                .await?;
            }

            database::jit_orders::insert(
                &mut ex,
                &jit_orders
                    .into_iter()
                    .map(|jit_order| database::jit_orders::JitOrder {
                        block_number,
                        log_index,
                        uid: ByteArray(jit_order.uid.0),
                        owner: ByteArray(jit_order.uid.owner().0 .0),
                        creation_timestamp: chrono::DateTime::from_timestamp(
                            i64::from(jit_order.created),
                            0,
                        )
                        .unwrap_or_default(),
                        sell_token: ByteArray(jit_order.sell.token.0 .0),
                        buy_token: ByteArray(jit_order.buy.token.0 .0),
                        sell_amount: u256_to_big_decimal(&jit_order.sell.amount.0),
                        buy_amount: u256_to_big_decimal(&jit_order.buy.amount.0),
                        valid_to: i64::from(jit_order.valid_to),
                        app_data: ByteArray(jit_order.app_data.0),
                        fee_amount: u256_to_big_decimal(&jit_order.fee_amount.0),
                        kind: match jit_order.side {
                            domain::auction::order::Side::Buy => database::orders::OrderKind::Buy,
                            domain::auction::order::Side::Sell => database::orders::OrderKind::Sell,
                        },
                        partially_fillable: jit_order.partially_fillable,
                        signature: jit_order.signature.to_bytes(),
                        receiver: ByteArray(jit_order.receiver.0 .0),
                        signing_scheme: match jit_order.signature.scheme() {
                            DomainSigningScheme::Eip712 => DbSigningScheme::Eip712,
                            DomainSigningScheme::EthSign => DbSigningScheme::EthSign,
                            DomainSigningScheme::Eip1271 => DbSigningScheme::Eip1271,
                            DomainSigningScheme::PreSign => DbSigningScheme::PreSign,
                        },
                        sell_token_balance: match jit_order.sell_token_balance {
                            DomainSellTokenSource::Erc20 => DbSellTokenSource::Erc20,
                            DomainSellTokenSource::External => DbSellTokenSource::External,
                            DomainSellTokenSource::Internal => DbSellTokenSource::Internal,
                        },
                        buy_token_balance: match jit_order.buy_token_balance {
                            DomainBuyTokenDestination::Erc20 => DbBuyTokenDestination::Erc20,
                            DomainBuyTokenDestination::Internal => DbBuyTokenDestination::Internal,
                        },
                    })
                    .collect::<Vec<_>>(),
            )
            .await?;
        }

        ex.commit().await?;
        Ok(())
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
#[error("failed communication with the database")]
pub struct DatabaseError(#[from] pub anyhow::Error);

impl From<sqlx::Error> for DatabaseError {
    fn from(err: sqlx::Error) -> Self {
        Self(err.into())
    }
}

pub mod error {
    use super::*;

    #[derive(Debug, thiserror::Error)]
    pub enum Auction {
        #[error("failed communication with the database: {0}")]
        DatabaseError(#[from] sqlx::Error),
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
        DatabaseError(#[from] sqlx::Error),
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
