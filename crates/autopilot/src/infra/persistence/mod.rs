use {
    crate::{
        boundary,
        database::{Postgres, order_events::store_order_events},
        domain::{self, eth, settlement::transaction::EncodedTrade},
        infra::persistence::dto::{AuctionId, RawAuctionData},
    },
    ::winner_selection::state::RankedItem,
    alloy::primitives::B256,
    anyhow::Context,
    bigdecimal::{BigDecimal, ToPrimitive},
    boundary::database::byte_array::ByteArray,
    chrono::{DateTime, Utc},
    database::{
        events::EventIndex,
        leader_pg_lock::LeaderLock,
        order_events::OrderEventLabel,
        order_execution::Asset,
        orders::{
            BuyTokenDestination as DbBuyTokenDestination,
            SellTokenSource as DbSellTokenSource,
            SigningScheme as DbSigningScheme,
        },
        solver_competition_v2::{self, Order, Solution},
    },
    domain::auction::order::{
        BuyTokenDestination as DomainBuyTokenDestination,
        SellTokenSource as DomainSellTokenSource,
        SigningScheme as DomainSigningScheme,
    },
    futures::{StreamExt, TryStreamExt},
    number::conversions::{big_decimal_to_u256, u256_to_big_decimal, u256_to_big_uint},
    shared::db_order_conversions::full_order_into_model_order,
    std::{
        collections::{HashMap, HashSet},
        ops::DerefMut,
        sync::Arc,
        time::Duration,
    },
    tokio::sync::mpsc,
    tracing::Instrument,
};

pub mod cli;
pub mod dto;

#[derive(Clone)]
pub struct Persistence {
    s3: Option<s3::Uploader>,
    postgres: Arc<Postgres>,
    /// Writing into this channel will cause the auction to be written to the
    /// DB in an orderly manner (FIFO).
    upload_queue: mpsc::UnboundedSender<AuctionUpload>,
}

struct AuctionUpload {
    auction_id: domain::auction::Id,
    /// Contains everything buy the auction_id.
    auction_data: RawAuctionData,
}

impl Persistence {
    pub async fn new(config: Option<s3::Config>, postgres: Arc<Postgres>) -> Self {
        let sender = Self::spawn_db_upload_task(postgres.clone());

        Self {
            s3: match config {
                Some(config) => Some(s3::Uploader::new(config).await),
                None => None,
            },
            postgres,
            upload_queue: sender,
        }
    }

    /// Spawns a task that writes the most recent auction to the DB. Uploads
    /// happen on a FIFO basis so the last write will always be the most
    /// recent auction.
    fn spawn_db_upload_task(db: Arc<Postgres>) -> mpsc::UnboundedSender<AuctionUpload> {
        let (sender, mut receiver) = mpsc::unbounded_channel::<AuctionUpload>();
        tokio::task::spawn(async move {
            while let Some(upload) = receiver.recv().await {
                if let Err(err) = db
                    .replace_current_auction(upload.auction_id, &upload.auction_data)
                    .await
                {
                    tracing::error!(?err, "failed to replace auction in DB");
                }
            }
            tracing::error!("auction upload task terminated unexpectedly");
        });
        sender
    }

    pub async fn leader(&self, key: String) -> LeaderLock {
        LeaderLock::new(self.postgres.pool.clone(), key, Duration::from_millis(200))
    }

    /// Spawns a background task that listens for new order notifications from
    /// PostgreSQL and notifies via the provided Notify.
    pub fn spawn_order_listener(&self, notify: Arc<tokio::sync::Notify>) {
        let pool = self.postgres.pool.clone();
        tokio::spawn(async move {
            loop {
                let mut listener = match sqlx::postgres::PgListener::connect_with(&pool).await {
                    Ok(listener) => listener,
                    Err(err) => {
                        tracing::error!(?err, "failed to create PostgreSQL listener");
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        continue;
                    }
                };

                tracing::info!("connected to PostgreSQL for order notifications");

                if let Err(err) = listener.listen("new_order").await {
                    tracing::error!(?err, "failed to listen on 'new_order' channel");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }

                loop {
                    match listener.recv().await {
                        Ok(notification) => {
                            let order_uid = notification.payload();
                            tracing::debug!(order_uid, "received order notification from postgres");
                            notify.notify_one();
                        }
                        Err(err) => {
                            tracing::error!(?err, "error receiving notification from postgres");
                            break;
                        }
                    }
                }
            }
        });
    }

    /// Fetches the ID that should be used for the next auction.
    pub async fn get_next_auction_id(&self) -> Result<domain::auction::Id, DatabaseError> {
        let _timer = Metrics::get()
            .database_queries
            .with_label_values(&["get_next_auction_id"])
            .start_timer();
        self.postgres
            .get_next_auction_id()
            .await
            .map_err(DatabaseError)
    }

    /// Spawns a background task that replaces the current auction in the DB
    /// with the new one.
    pub fn replace_current_auction_in_db(
        &self,
        new_auction_id: domain::auction::Id,
        new_auction_data: &domain::RawAuctionData,
    ) {
        self.upload_queue
            .send(AuctionUpload {
                auction_id: new_auction_id,
                auction_data: dto::auction::from_domain(new_auction_data.clone()),
            })
            .expect("upload queue should be alive at all times");
    }

    /// Spawns a background task that uploads the auction to S3.
    pub fn upload_auction_to_s3(&self, id: domain::auction::Id, auction: &domain::RawAuctionData) {
        if auction.orders.is_empty() {
            return;
        }
        let Some(s3) = self.s3.clone() else {
            return;
        };
        let auction_dto = dto::auction::from_domain(auction.clone());
        tokio::task::spawn(async move {
            match s3.upload(id.to_string(), &auction_dto).await {
                Ok(key) => tracing::info!(?key, "uploaded auction to s3"),
                Err(err) => tracing::warn!(?err, "failed to upload auction to s3"),
            }
        });
    }

    /// Finds solvable orders based on the order's min validity period.
    pub async fn all_solvable_orders(
        &self,
        min_valid_to: u32,
    ) -> anyhow::Result<boundary::SolvableOrders> {
        self.postgres
            .all_solvable_orders(min_valid_to)
            .await
            .context("failed to fetch all solvable orders")
    }

    /// Saves the competition data to the DB
    pub async fn save_competition(
        &self,
        competition: boundary::Competition,
    ) -> Result<(), DatabaseError> {
        self.postgres
            .save_competition(competition)
            .await
            .map_err(DatabaseError)
    }

    /// Save all valid solutions that participated in the competition for an
    /// auction.
    pub async fn save_solutions(
        &self,
        auction_id: domain::auction::Id,
        solutions: impl Iterator<Item = &domain::competition::Bid>,
    ) -> Result<(), DatabaseError> {
        let _timer = Metrics::get()
            .database_queries
            .with_label_values(&["save_solutions"])
            .start_timer();

        let mut ex = self.postgres.pool.begin().await?;

        database::solver_competition_v2::save(
            &mut ex,
            auction_id,
            &solutions
                .enumerate()
                .map(|(uid, bid)| {
                    let solution = Solution {
                        uid: uid.try_into().context("uid overflow")?,
                        id: BigDecimal::from(bid.solution().id()),
                        solver: ByteArray(bid.solution().solver().0.0),
                        is_winner: bid.is_winner(),
                        filtered_out: bid.is_filtered_out(),
                        score: u256_to_big_decimal(&bid.score().get().0),
                        orders: bid
                            .solution()
                            .orders()
                            .iter()
                            .map(|(order_uid, order)| Order {
                                uid: ByteArray(order_uid.0),
                                sell_token: ByteArray(order.sell.token.0.0.0),
                                buy_token: ByteArray(order.buy.token.0.0.0),
                                limit_sell: u256_to_big_decimal(&order.sell.amount.0),
                                limit_buy: u256_to_big_decimal(&order.buy.amount.0),
                                executed_sell: u256_to_big_decimal(&order.executed_sell.0),
                                executed_buy: u256_to_big_decimal(&order.executed_buy.0),
                                side: order.side.into(),
                            })
                            .collect(),
                        price_tokens: bid
                            .solution()
                            .prices()
                            .keys()
                            .map(|token| ByteArray(token.0.0.0))
                            .collect(),
                        price_values: bid
                            .solution()
                            .prices()
                            .values()
                            .map(|price| u256_to_big_decimal(&price.get().0))
                            .collect(),
                    };
                    Ok::<_, DatabaseError>(solution)
                })
                .collect::<Result<Vec<_>, DatabaseError>>()?,
        )
        .await?;

        Ok(ex.commit().await?)
    }

    /// Saves the surplus capturing jit order owners to the DB
    pub async fn save_surplus_capturing_jit_order_owners(
        &self,
        auction_id: AuctionId,
        surplus_capturing_jit_order_owners: &[domain::eth::Address],
    ) -> Result<(), DatabaseError> {
        self.postgres
            .save_surplus_capturing_jit_order_owners(
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
        order_uids: impl IntoIterator<Item = domain::OrderUid>,
        label: boundary::OrderEventLabel,
    ) {
        let db = self.postgres.clone();
        let order_uids = order_uids.into_iter().collect();
        tokio::spawn(
            async move {
                match db.pool.acquire().await {
                    Ok(mut tx) => {
                        store_order_events(&mut tx, order_uids, label, Utc::now()).await;
                    }
                    Err(err) => {
                        tracing::error!(
                            ?err,
                            "failed to acquire a connection to store order events!"
                        );
                    }
                };
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
        let _timer = Metrics::get()
            .database_queries
            .with_label_values(&["store_fee_policies"])
            .start_timer();

        let mut ex = self.postgres.pool.begin().await.context("begin")?;
        for chunk in fee_policies.chunks(self.postgres.config.insert_batch_size.get()) {
            crate::database::fee_policies::insert_batch(&mut ex, auction_id, chunk.iter().cloned())
                .await
                .context("fee_policies::insert_batch")?;
        }

        ex.commit().await.context("commit")
    }

    /// Tries to find the transaction executing a given solution proposed
    /// by the solver.
    pub async fn find_settlement_transaction(
        &self,
        auction_id: i64,
        solver: eth::Address,
        solution_uid: usize,
    ) -> Result<Option<eth::TxId>, DatabaseError> {
        let _timer = Metrics::get()
            .database_queries
            .with_label_values(&["find_settlement_transaction"])
            .start_timer();

        let mut ex = self.postgres.pool.acquire().await.context("acquire")?;
        Ok(database::settlements::find_settlement_transaction(
            &mut ex,
            auction_id,
            ByteArray(solver.0.0),
            solution_uid
                .try_into()
                .context("could not convert solution id to i64")?,
        )
        .await?
        .map(|hash| B256::new(hash.0).into()))
    }

    /// Save auction related data to the database.
    pub async fn save_auction(
        &self,
        auction: &domain::Auction,
        deadline: u64, // to become part of the auction struct
    ) -> Result<(), DatabaseError> {
        let _timer = Metrics::get()
            .database_queries
            .with_label_values(&["save_auction"])
            .start_timer();

        let mut ex = self.postgres.pool.acquire().await?;

        database::auction::save(
            &mut ex,
            database::auction::Auction {
                id: auction.id,
                block: i64::try_from(auction.block).context("block overflow")?,
                deadline: i64::try_from(deadline).context("deadline overflow")?,
                order_uids: auction
                    .orders
                    .iter()
                    .map(|order| ByteArray(order.uid.0))
                    .collect(),
                price_tokens: auction
                    .prices
                    .keys()
                    .map(|token| ByteArray(token.0.0.0))
                    .collect(),
                price_values: auction
                    .prices
                    .values()
                    .map(|price| u256_to_big_decimal(&price.get().0))
                    .collect(),
                surplus_capturing_jit_order_owners: auction
                    .surplus_capturing_jit_order_owners
                    .iter()
                    .map(|owner| ByteArray(owner.0.0))
                    .collect(),
            },
        )
        .await?;

        Ok(())
    }

    /// Get auction data to post-process the given trades.
    pub async fn get_auction(
        &self,
        auction_id: domain::auction::Id,
        trades: &[EncodedTrade],
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
                .map(|owner| eth::Address::new(owner.0))
                .collect();

        let prices = database::auction_prices::fetch(&mut ex, auction_id)
            .await
            .map_err(error::Auction::DatabaseError)?
            .into_iter()
            .map(|price| {
                let token = eth::Address::new(price.token.0).into();
                let price = big_decimal_to_u256(&price.price)
                    .ok_or(domain::auction::InvalidPrice)
                    .and_then(|p| domain::auction::Price::try_new(p.into()))
                    .map_err(|_err| error::Auction::InvalidPrice(token));
                price.map(|price| (token, price))
            })
            .collect::<Result<_, _>>()?;

        let orders = {
            let auction_orders = database::auction::get_order_uids(&mut ex, auction_id)
                .await
                .map_err(error::Auction::DatabaseError)?
                .ok_or(error::Auction::NotFound)?
                .into_iter()
                .map(|order| domain::OrderUid(order.0))
                .collect::<HashSet<_>>();
            // Code that uses the data assembled by this function determines JIT orders
            // by their presence in the `orders => fee_policies` mapping. If an order has
            // a mapping it is assumed that this was a regular order and not a JIT order.
            // So in order to not misclassify JIT orders as regular orders we only fetch
            // fee policies for orders that were part of the original auction.
            let relevant_orders: HashSet<_> = trades
                .iter()
                .filter(|t| auction_orders.contains(&t.uid))
                .map(|t| t.uid)
                .collect();

            // get fee policies for all orders that were part of the competition auction
            let fee_policies = database::fee_policies::fetch_all(
                &mut ex,
                relevant_orders
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
            for order in relevant_orders.iter() {
                let order_policies = match fee_policies.get(order) {
                    Some(policies) => policies
                        .iter()
                        .cloned()
                        .map(|policy| {
                            dto::fee_policy::try_into_domain(
                                policy,
                                quotes.get(order).map(|v| &**v),
                            )
                            .map_err(|err| error::Auction::InvalidFeePolicy(err, *order))
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                    None => vec![],
                };
                orders.insert(*order, order_policies);
            }
            orders
        };

        let block = {
            let block = database::solver_competition::auction_start_block(&mut ex, auction_id)
                .await?
                .ok_or(error::Auction::NotFound)?;
            block
                .parse::<u64>()
                .map_err(|_| error::Auction::NotFound)?
                .into()
        };

        Ok(domain::settlement::Auction {
            id: auction_id,
            block,
            orders,
            prices,
            surplus_capturing_jit_order_owners,
        })
    }

    /// Computes solvable orders based on the latest observed block number,
    /// order creation timestamp, and minimum validity period.
    pub async fn solvable_orders_after(
        &self,
        mut current_orders: HashMap<domain::OrderUid, Arc<model::order::Order>>,
        mut current_quotes: HashMap<domain::OrderUid, Arc<domain::Quote>>,
        after_timestamp: DateTime<Utc>,
        after_block: u64,
        min_valid_to: u32,
    ) -> anyhow::Result<boundary::SolvableOrders> {
        tracing::debug!(?after_timestamp, ?after_block, "fetch orders updated since");
        let after_block = i64::try_from(after_block).context("block number value exceeds i64")?;
        let started_at = chrono::offset::Utc::now();
        let mut tx = self.postgres.pool.begin().await.context("begin")?;
        // Set the transaction isolation level to REPEATABLE READ
        // so all the SELECT queries below are executed in the same database snapshot
        // taken at the moment before the first query is executed.
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
            .execute(tx.deref_mut())
            .await?;

        // Find order uids for orders that were updated after the given block.
        let updated_order_uids = {
            let _timer = Metrics::get()
                .database_queries
                .with_label_values(&["updated_order_uids"])
                .start_timer();

            database::orders::updated_order_uids_after(&mut tx, after_block).await?
        };

        // Fetch the orders that were updated after the given block and were created or
        // cancelled after the given timestamp.
        let next_orders: HashMap<domain::OrderUid, Arc<model::order::Order>> = {
            let _timer = Metrics::get()
                .database_queries
                .with_label_values(&["open_orders_after"])
                .start_timer();

            database::orders::open_orders_by_time_or_uids(
                &mut tx,
                &updated_order_uids,
                after_timestamp,
            )
            .map(|result| match result {
                Ok(order) => full_order_into_model_order(order)
                    .map(|order| (domain::OrderUid(order.metadata.uid.0), Arc::new(order))),
                Err(err) => Err(anyhow::Error::from(err)),
            })
            .try_collect()
            .await?
        };

        let latest_settlement_block = database::orders::latest_settlement_block(&mut tx)
            .await?
            .to_u64()
            .context("latest_settlement_block is not u64")?;

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

        current_quotes.retain(|uid, _| current_orders.contains_key(uid));

        {
            let _timer = Metrics::get()
                .database_queries
                .with_label_values(&["read_quotes"])
                .start_timer();

            // Fetch quotes only for newly created and also on-chain placed orders due to
            // the following case: if a block containing an on-chain order
            // (e.g., ethflow) gets reorganized, the same order with the same
            // UID might be created in the new block, and the temporary quote
            // associated with it may have changed in the meantime.
            let order_uids = current_orders
                .values()
                .filter_map(|order| {
                    (order.metadata.onchain_user.is_some()
                        || order.metadata.creation_date > after_timestamp)
                        .then_some(ByteArray(order.metadata.uid.0))
                })
                .collect::<Vec<_>>();

            for quote in database::orders::read_quotes(&mut tx, &order_uids).await? {
                let order_uid = domain::OrderUid(quote.order_uid.0);
                match dto::quote::into_domain(quote) {
                    Ok(quote) => {
                        current_quotes.insert(order_uid, Arc::new(quote));
                    }
                    Err(err) => tracing::warn!(?order_uid, ?err, "failed to convert quote from db"),
                }
            }
        };

        Ok(boundary::SolvableOrders {
            orders: current_orders,
            quotes: current_quotes,
            latest_settlement_block,
            fetched_from_db: started_at,
        })
    }

    /// Returns the oldest settlement event for which the accociated auction is
    /// not yet populated in the database.
    pub async fn get_settlements_without_auction(
        &self,
    ) -> Result<Vec<domain::eth::SettlementEvent>, DatabaseError> {
        let _timer = Metrics::get()
            .database_queries
            .with_label_values(&["get_settlement_without_auction"])
            .start_timer();

        let mut ex = self.postgres.pool.acquire().await?;
        let events = database::settlements::get_settlements_without_auction(&mut ex)
            .await?
            .into_iter()
            .map(|event| {
                let event = domain::eth::SettlementEvent {
                    block: u64::try_from(event.block_number)
                        .context("negative block")?
                        .into(),
                    log_index: u64::try_from(event.log_index).context("negative log index")?,
                    transaction: eth::TxId(B256::new(event.tx_hash.0)),
                };
                Ok::<_, DatabaseError>(event)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(events)
    }

    /// Returns the trade events that are associated with the settlement event
    pub async fn get_trades_for_settlement(
        &self,
        settlement: &domain::eth::SettlementEvent,
    ) -> Result<Vec<domain::eth::TradeEvent>, DatabaseError> {
        let _timer = Metrics::get()
            .database_queries
            .with_label_values(&["get_trades_for_settlement"])
            .start_timer();

        let mut ex = self.postgres.pool.acquire().await?;
        database::trades::get_trades_for_settlement(
            &mut ex,
            EventIndex {
                block_number: i64::try_from(settlement.block.0).context("block overflow")?,
                log_index: i64::try_from(settlement.log_index).context("log index overflow")?,
            },
        )
        .await?
        .into_iter()
        .map(|event| {
            let event = domain::eth::TradeEvent {
                block: u64::try_from(event.block_number)
                    .context("negative block")?
                    .into(),
                log_index: u64::try_from(event.log_index).context("negative log index")?,
                order_uid: domain::OrderUid(event.order_uid.0),
            };
            Ok::<_, DatabaseError>(event)
        })
        .collect()
    }

    pub async fn save_settlement(
        &self,
        event: domain::eth::SettlementEvent,
        settlement: Option<&domain::settlement::Settlement>,
    ) -> Result<(), DatabaseError> {
        let _timer = Metrics::get()
            .database_queries
            .with_label_values(&["save_settlement"])
            .start_timer();

        let mut ex = self.postgres.pool.begin().await?;

        let block_number = i64::try_from(event.block.0).context("block overflow")?;
        let log_index = i64::try_from(event.log_index).context("log index overflow")?;
        let auction_id = settlement.map(|s| s.auction_id()).unwrap_or_default();
        tracing::debug!(hash = ?event.transaction, ?auction_id, "saving settlement details for tx");

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
            let fee_breakdown = settlement.fee_breakdown();
            let jit_orders = settlement.jit_orders();
            let solver: database::Address = ByteArray(settlement.solver().0.0);

            tracing::debug!(
                ?auction_id,
                hash = ?event.transaction,
                ?solver,
                ?gas,
                ?gas_price,
                ?surplus,
                ?fee,
                ?fee_breakdown,
                ?jit_orders,
                "settlement update",
            );

            database::settlements::update_settlement_solver(
                &mut ex,
                block_number,
                log_index,
                solver,
                settlement.solution_uid(),
            )
            .await?;

            store_order_events(
                &mut ex,
                fee_breakdown.keys().cloned().collect(),
                OrderEventLabel::Traded,
                Utc::now(),
            )
            .await;

            for (order, order_fee) in fee_breakdown {
                database::order_execution::save(
                    &mut ex,
                    &ByteArray(order.0),
                    auction_id,
                    block_number,
                    Asset {
                        token: ByteArray(order_fee.total.token.0.0.0),
                        amount: u256_to_big_decimal(&order_fee.total.amount.0),
                    },
                    &order_fee
                        .protocol
                        .into_iter()
                        .map(|executed| Asset {
                            token: ByteArray(executed.fee.token.0.0.0),
                            amount: u256_to_big_decimal(&executed.fee.amount.0),
                        })
                        .collect::<Vec<_>>(),
                )
                .await?;
            }

            if !jit_orders.is_empty() {
                // each jit order should have a corresponding trade event, try to find them
                let trade_events = self
                    .get_trades_for_settlement(&event)
                    .await?
                    .into_iter()
                    .map(|event| (event.order_uid, (event.block, event.log_index)))
                    .collect::<HashMap<_, (_, _)>>();

                database::jit_orders::insert(
                    &mut ex,
                    &jit_orders
                        .into_iter()
                        .filter_map(|jit_order| match trade_events.get(&jit_order.uid) {
                            Some((block_number, log_index)) => {
                                Some(database::jit_orders::JitOrder {
                                    block_number: i64::try_from(block_number.0).ok()?,
                                    log_index: i64::try_from(*log_index).ok()?,
                                    uid: ByteArray(jit_order.uid.0),
                                    owner: ByteArray(jit_order.uid.owner().0.0),
                                    creation_timestamp: chrono::DateTime::from_timestamp(
                                        i64::from(jit_order.created),
                                        0,
                                    )
                                        .unwrap_or_default(),
                                    sell_token: ByteArray(jit_order.sell.token.0.0.0),
                                    buy_token: ByteArray(jit_order.buy.token.0.0.0),
                                    sell_amount: u256_to_big_decimal(&jit_order.sell.amount.0),
                                    buy_amount: u256_to_big_decimal(&jit_order.buy.amount.0),
                                    valid_to: i64::from(jit_order.valid_to),
                                    app_data: ByteArray(jit_order.app_data.0),
                                    fee_amount: u256_to_big_decimal(&jit_order.fee_amount.0),
                                    kind: jit_order.side.into(),
                                    partially_fillable: jit_order.partially_fillable,
                                    signature: jit_order.signature.to_bytes(),
                                    receiver: ByteArray(jit_order.receiver.0.0),
                                    signing_scheme: match jit_order.signature.scheme() {
                                        DomainSigningScheme::Eip712 => DbSigningScheme::Eip712,
                                        DomainSigningScheme::EthSign => DbSigningScheme::EthSign,
                                        DomainSigningScheme::Eip1271 => DbSigningScheme::Eip1271,
                                        DomainSigningScheme::PreSign => DbSigningScheme::PreSign,
                                    },
                                    sell_token_balance: match jit_order.sell_token_balance {
                                        DomainSellTokenSource::Erc20 => DbSellTokenSource::Erc20,
                                        DomainSellTokenSource::External => {
                                            DbSellTokenSource::External
                                        }
                                        DomainSellTokenSource::Internal => {
                                            DbSellTokenSource::Internal
                                        }
                                    },
                                    buy_token_balance: match jit_order.buy_token_balance {
                                        DomainBuyTokenDestination::Erc20 => {
                                            DbBuyTokenDestination::Erc20
                                        }
                                        DomainBuyTokenDestination::Internal => {
                                            DbBuyTokenDestination::Internal
                                        }
                                    },
                                })
                            }
                            None => {
                                tracing::warn!(order_uid = ?jit_order.uid, "missing trade event for jit order");
                                None
                            }
                        })
                        .collect::<Vec<_>>(),
                )
                    .await?;
            }
        }

        ex.commit().await?;
        Ok(())
    }

    pub async fn store_settlement_execution_started(
        &self,
        event: domain::settlement::ExecutionStarted,
    ) -> Result<(), DatabaseError> {
        let mut ex = self.postgres.pool.acquire().await.context("acquire")?;
        let _timer = Metrics::get()
            .database_queries
            .with_label_values(&["insert_settlement_execution_event"])
            .start_timer();

        database::settlement_executions::insert(
            &mut ex,
            event.auction_id,
            ByteArray(event.solver.0.0),
            event
                .solution_uid
                .try_into()
                .context("solution uid overflow")?,
            event.start_timestamp,
            event
                .start_block
                .try_into()
                .context("start block overflow")?,
            event
                .deadline_block
                .try_into()
                .context("deadline block overflow")?,
        )
        .await?;

        Ok(())
    }

    pub async fn store_settlement_execution_ended(
        &self,
        event: domain::settlement::ExecutionEnded,
    ) -> Result<(), DatabaseError> {
        let mut ex = self.postgres.pool.acquire().await.context("acquire")?;
        let _timer = Metrics::get()
            .database_queries
            .with_label_values(&["update_settlement_execution_event"])
            .start_timer();

        database::settlement_executions::update(
            &mut ex,
            event.auction_id,
            ByteArray(event.solver.0.0),
            event
                .solution_uid
                .try_into()
                .context("solution uid overflow")?,
            event.end_timestamp,
            event.end_block.try_into().context("end block overflow")?,
            event.outcome,
        )
        .await?;

        Ok(())
    }

    pub async fn get_solver_winning_solutions(
        &self,
        auction_id: domain::auction::Id,
        solver: eth::Address,
    ) -> Result<Vec<Solution>, DatabaseError> {
        let mut ex = self.postgres.pool.acquire().await.context("acquire")?;
        let _timer = Metrics::get()
            .database_queries
            .with_label_values(&["fetch_solver_winning_solutions"])
            .start_timer();

        Ok(
            database::solver_competition_v2::fetch_solver_winning_solutions(
                &mut ex,
                auction_id,
                ByteArray(solver.0.0),
            )
            .await
            .context("solver_competition::fetch_solver_winning_solutions")?,
        )
    }

    /// Fetches orders which are currently inflight. Those orders should
    /// be omitted from the current auction to avoid onchain reverts.
    pub async fn fetch_in_flight_orders(
        &self,
        current_block: u64,
    ) -> anyhow::Result<HashSet<crate::domain::OrderUid>> {
        let _timer = Metrics::get()
            .database_queries
            .with_label_values(&["inflight_orders"])
            .start_timer();

        let mut ex = self.postgres.pool.acquire().await.context("acquire")?;
        let orders =
            solver_competition_v2::fetch_in_flight_orders(&mut ex, current_block.cast_signed())
                .await?;
        Ok(orders
            .into_iter()
            .map(|o| crate::domain::OrderUid(o.0))
            .collect())
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
}
