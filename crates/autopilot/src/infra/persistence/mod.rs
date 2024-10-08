use {
    crate::{
        boundary,
        database::{order_events::store_order_events, Postgres},
        domain::{self, eth},
        infra::persistence::dto::AuctionId,
    },
    anyhow::Context,
    bigdecimal::ToPrimitive,
    boundary::database::byte_array::ByteArray,
    chrono::{DateTime, Utc},
    database::{
        events::EventIndex,
        order_events::OrderEventLabel,
        order_execution::Asset,
        orders::{
            BuyTokenDestination as DbBuyTokenDestination,
            SellTokenSource as DbSellTokenSource,
            SigningScheme as DbSigningScheme,
        },
        settlement_observations::Observation,
        solver_competition::{Order, Solution},
    },
    domain::auction::order::{
        BuyTokenDestination as DomainBuyTokenDestination,
        SellTokenSource as DomainSellTokenSource,
        SigningScheme as DomainSigningScheme,
    },
    futures::{StreamExt, TryStreamExt},
    number::conversions::{big_decimal_to_u256, u256_to_big_decimal, u256_to_big_uint},
    primitive_types::H256,
    shared::db_order_conversions::full_order_into_model_order,
    std::{
        collections::{HashMap, HashSet, VecDeque},
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
        auction: &domain::RawAuctionData,
    ) -> Result<domain::auction::Id, DatabaseError> {
        let auction = dto::auction::from_domain(auction.clone());
        self.postgres
            .replace_current_auction(&auction)
            .await
            .inspect(|&id| {
                self.archive_auction(dto::auction::Auction { id, auction });
            })
            .map_err(DatabaseError)
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

    /// Saves the given auction to storage for debugging purposes.
    ///
    /// There is no intention to retrieve this data programmatically.
    fn archive_auction(&self, instance: dto::auction::Auction) {
        let Some(uploader) = self.s3.clone() else {
            return;
        };
        if instance.auction.orders.is_empty() {
            tracing::info!("skip upload of empty auction");
            return;
        }
        tokio::spawn(
            async move {
                match uploader
                    .upload(instance.id.to_string(), &instance.auction)
                    .await
                {
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

    /// Save all valid solutions that participated in the competition for an
    /// auction.
    pub async fn save_solutions(
        &self,
        auction_id: domain::auction::Id,
        solutions: &[domain::competition::Participant],
    ) -> Result<(), DatabaseError> {
        let _timer = Metrics::get()
            .database_queries
            .with_label_values(&["save_solutions"])
            .start_timer();

        let mut ex = self.postgres.pool.begin().await?;

        database::solver_competition::save(
            &mut ex,
            auction_id,
            &solutions
                .iter()
                .enumerate()
                .map(|(uid, participant)| {
                    let solution = Solution {
                        uid: uid.try_into().context("uid overflow")?,
                        id: i64::try_from(participant.solution().id()).context("block overflow")?,
                        solver: ByteArray(participant.solution().solver().0 .0),
                        is_winner: participant.is_winner(),
                        score: u256_to_big_decimal(&participant.solution().score().get().0),
                        orders: participant
                            .solution()
                            .orders()
                            .iter()
                            .map(|(order_uid, order)| Order {
                                uid: ByteArray(order_uid.0),
                                sell_token: ByteArray(order.sell.token.0 .0),
                                buy_token: ByteArray(order.buy.token.0 .0),
                                limit_sell: u256_to_big_decimal(&order.sell.amount.0),
                                limit_buy: u256_to_big_decimal(&order.buy.amount.0),
                                executed_sell: u256_to_big_decimal(&order.executed_sell.0),
                                executed_buy: u256_to_big_decimal(&order.executed_buy.0),
                                side: order.side.into(),
                            })
                            .collect(),
                        price_tokens: participant
                            .solution()
                            .prices()
                            .keys()
                            .map(|token| ByteArray(token.0 .0))
                            .collect(),
                        price_values: participant
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

    /// For a given auction and solver, tries to find the settlement
    /// transaction.
    pub async fn find_settlement_transaction(
        &self,
        auction_id: i64,
        solver: eth::Address,
    ) -> Result<Option<eth::TxId>, DatabaseError> {
        let _timer = Metrics::get()
            .database_queries
            .with_label_values(&["find_settlement_transaction"])
            .start_timer();

        let mut ex = self.postgres.pool.acquire().await.context("acquire")?;
        Ok(database::settlements::find_settlement_transaction(
            &mut ex,
            auction_id,
            ByteArray(solver.0 .0),
        )
        .await?
        .map(|hash| H256(hash.0).into()))
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

        let mut ex = self.postgres.pool.begin().await?;

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
                    .map(|token| ByteArray(token.0 .0))
                    .collect(),
                price_values: auction
                    .prices
                    .values()
                    .map(|price| u256_to_big_decimal(&price.get().0))
                    .collect(),
                surplus_capturing_jit_order_owners: auction
                    .surplus_capturing_jit_order_owners
                    .iter()
                    .map(|owner| ByteArray(owner.0 .0))
                    .collect(),
            },
        )
        .await?;

        Ok(ex.commit().await?)
    }

    /// Save auction related data to the database.
    pub async fn save_auction(
        &self,
        auction_id: domain::auction::Id,
        auction: &domain::Auction,
        deadline: u64, // to become part of the auction struct
    ) -> Result<(), DatabaseError> {
        let _timer = Metrics::get()
            .database_queries
            .with_label_values(&["save_auction"])
            .start_timer();

        let mut ex = self.postgres.pool.begin().await?;

        database::auction::save(
            &mut ex,
            database::auction::Auction {
                id: auction_id,
                block: i64::try_from(auction.block).context("block overflow")?,
                deadline: i64::try_from(deadline).context("deadline overflow")?,
                orders: auction
                    .orders
                    .iter()
                    .map(|order| ByteArray(order.uid.0))
                    .collect(),
                price_tokens: auction
                    .prices
                    .keys()
                    .map(|token| ByteArray(token.0 .0))
                    .collect(),
                price_values: auction
                    .prices
                    .values()
                    .map(|price| u256_to_big_decimal(&price.get().0))
                    .collect(),
                surplus_capturing_jit_order_owners: auction
                    .surplus_capturing_jit_order_owners
                    .iter()
                    .map(|owner| ByteArray(owner.0 .0))
                    .collect(),
            },
        )
        .await?;

        Ok(ex.commit().await?)
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

        let block = {
            let competition = database::solver_competition::load_by_id(&mut ex, auction_id)
                .await?
                .ok_or(error::Auction::NotFound)?;
            serde_json::from_value::<boundary::SolverCompetitionDB>(competition.json)
                .map_err(|_| error::Auction::NotFound)?
                .auction_start_block
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
        mut current_orders: HashMap<domain::OrderUid, model::order::Order>,
        mut current_quotes: HashMap<domain::OrderUid, domain::Quote>,
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
        let next_orders: HashMap<domain::OrderUid, model::order::Order> = {
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
                    .map(|order| (domain::OrderUid(order.metadata.uid.0), order)),
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
                        current_quotes.insert(order_uid, quote);
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
    pub async fn get_settlement_without_auction(
        &self,
    ) -> Result<Option<domain::eth::SettlementEvent>, DatabaseError> {
        let _timer = Metrics::get()
            .database_queries
            .with_label_values(&["get_settlement_without_auction"])
            .start_timer();

        let mut ex = self.postgres.pool.acquire().await?;
        let event = database::settlements::get_settlement_without_auction(&mut ex)
            .await?
            .map(|event| {
                let event = domain::eth::SettlementEvent {
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
            let fee_breakdown = settlement.fee_breakdown();
            let jit_orders = settlement.jit_orders();

            tracing::debug!(
                ?auction_id,
                hash = ?event.transaction,
                ?gas,
                ?gas_price,
                ?surplus,
                ?fee,
                ?fee_breakdown,
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
                fee_breakdown.keys().cloned().collect(),
                OrderEventLabel::Traded,
                Utc::now(),
            )
            .await;

            for (order, order_fee) in fee_breakdown {
                let total_fee = order_fee
                    .as_ref()
                    .map(|fee| u256_to_big_decimal(&fee.total.0))
                    .unwrap_or_default();
                let executed_protocol_fees = order_fee
                    .map(|fee| {
                        fee.protocol
                            .into_iter()
                            .map(|executed| Asset {
                                token: ByteArray(executed.fee.token.0 .0),
                                amount: u256_to_big_decimal(&executed.fee.amount.0),
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                database::order_execution::save(
                    &mut ex,
                    &ByteArray(order.0),
                    auction_id,
                    block_number,
                    &total_fee,
                    &executed_protocol_fees,
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
                                    kind: jit_order.side.into(),
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

    // todo delete
    /// A one time migration from solver competition table to auctions table.
    pub async fn migrate_solver_competitions(&self) -> Result<(), DatabaseError> {
        let mut ex = self.postgres.pool.begin().await?;

        for solver_competition in database::solver_competition::all(&mut ex).await? {
            let competition: model::solver_competition::SolverCompetitionDB =
                serde_json::from_value(solver_competition.json)
                    .context("deserialize SolverCompetitionDB")?;

            // populate historic auctions
            let auction = database::auction::Auction {
                id: solver_competition.id,
                block: i64::try_from(competition.auction_start_block).context("block overflow")?,
                deadline: solver_competition.deadline,
                order_uids: competition
                    .auction
                    .orders
                    .iter()
                    .map(|order| ByteArray(order.0))
                    .collect(),
                price_tokens: competition
                    .auction
                    .prices
                    .keys()
                    .map(|token| ByteArray(token.0))
                    .collect(),
                price_values: competition
                    .auction
                    .prices
                    .values()
                    .map(u256_to_big_decimal)
                    .collect(),
                surplus_capturing_jit_order_owners: solver_competition
                    .surplus_capturing_jit_order_owners,
            };

            if let Err(err) = database::auction::save(&mut ex, auction).await {
                tracing::warn!(?err, auction_id = ?solver_competition.id, "failed to save auction");
            }

            // populate historic solutions

            // todo: implement
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
}
