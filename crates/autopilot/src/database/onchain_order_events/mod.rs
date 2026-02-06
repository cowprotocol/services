pub mod ethflow_events;
pub mod event_retriever;

use {
    super::{Metrics as DatabaseMetrics, Postgres, events::bytes_to_order_uid},
    crate::database::events::log_to_event_index,
    alloy::{
        eips::BlockNumberOrTag,
        primitives::{Address, TxHash, U256},
        rpc::types::Log,
    },
    anyhow::{Context, Result, anyhow, bail},
    app_data::AppDataHash,
    chrono::{TimeZone, Utc},
    contracts::alloy::{
        CoWSwapOnchainOrders::CoWSwapOnchainOrders::{
            CoWSwapOnchainOrdersEvents as ContractEvent,
            OrderInvalidation,
            OrderPlacement as ContractOrderPlacement,
        },
        HooksTrampoline::{self, HooksTrampoline::Hook},
    },
    database::{
        PgTransaction,
        byte_array::ByteArray,
        events::EventIndex,
        onchain_broadcasted_orders::{OnchainOrderPlacement, OnchainOrderPlacementError},
        orders::{Order, OrderClass, insert_quotes},
    },
    ethrpc::{
        Web3,
        block_stream::{RangeInclusive, timestamp_of_block_in_seconds},
    },
    futures::{StreamExt, stream},
    itertools::{izip, multiunzip},
    model::{
        DomainSeparator,
        order::{
            BuyTokenDestination,
            OrderData,
            OrderKind,
            OrderUid,
            QuoteAmounts,
            SellTokenSource,
        },
        signature::SigningScheme,
    },
    number::conversions::u256_to_big_decimal,
    shared::{
        db_order_conversions::{
            buy_token_destination_into,
            order_kind_into,
            sell_token_source_into,
            signing_scheme_into,
        },
        event_handling::EventStoring,
        order_quoting::{OrderQuoting, Quote, QuoteSearchParameters},
        order_validation::{
            ValidationError,
            convert_signing_scheme_into_quote_signing_scheme,
            get_quote_and_check_fee,
        },
    },
    sqlx::PgConnection,
    std::{collections::HashMap, sync::Arc},
};

pub struct OnchainOrderParser<EventData: Send + Sync, EventRow: Send + Sync> {
    db: Postgres,
    web3: Web3,
    quoter: Arc<dyn OrderQuoting>,
    custom_onchain_data_parser: Box<dyn OnchainOrderParsing<EventData, EventRow>>,
    domain_separator: DomainSeparator,
    settlement_contract: Address,
    metrics: &'static Metrics,
    trampoline: HooksTrampoline::Instance,
}

impl<EventData, EventRow> OnchainOrderParser<EventData, EventRow>
where
    EventData: Send + Sync,
    EventRow: Send + Sync,
{
    pub fn new(
        db: Postgres,
        web3: Web3,
        quoter: Arc<dyn OrderQuoting>,
        custom_onchain_data_parser: Box<dyn OnchainOrderParsing<EventData, EventRow>>,
        domain_separator: DomainSeparator,
        settlement_contract: Address,
        trampoline: HooksTrampoline::Instance,
    ) -> Self {
        OnchainOrderParser {
            db,
            web3,
            quoter,
            custom_onchain_data_parser,
            domain_separator,
            settlement_contract,
            metrics: Metrics::get(),
            trampoline,
        }
    }
}

// The following struct describes the return type from the custom order parsing
// logic. All parser must return a quote_id, as this is currently required by
// the protocol.
pub struct OnchainOrderCustomData<T> {
    quote_id: i64,
    additional_data: Option<T>,
}

// The following trait allows to implement custom onchain order parsing for
// differently placed orders. E.g., there will be a implementation for ethflow
// and presign orders. For each of the customs types, the trait allows to
// implement parsing the on-chain data and storing the event data

// The generic EventData stores the result of the custom event parsing
// The generic EvenDataForDB contains the prepared data that will be appended
// to the database
#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait OnchainOrderParsing<EventData, EventRow>: Send + Sync
where
    EventData: Send + Sync + Clone,
    EventRow: Send + Sync,
{
    // This function allows each implementaiton to store custom data to the
    // database
    async fn append_custom_order_info_to_db<'a>(
        &self,
        ex: &mut PgTransaction<'a>,
        custom_onchain_data: Vec<EventRow>,
    ) -> Result<()>;

    // This function allows to parse each event contract differently

    // The implementaiton is expected to not error on normal parsing errors
    // Events for which a regular parsing error happens should just be dropped
    // Errors that are unexpected / non-recoverable should be returned as errors
    fn parse_custom_event_data(
        &self,
        contract_events: &[(ContractEvent, Log)],
    ) -> Result<Vec<(EventIndex, OnchainOrderCustomData<EventData>)>>;

    // This function allow to create the specific object that will be stored in
    // the database by the fn append_custo_order_info_to_db
    fn customized_event_data_for_event_index(
        &self,
        event_index: &EventIndex,
        order: &Order,
        hashmap: &HashMap<EventIndex, EventData>,
        onchain_order_placement: &OnchainOrderPlacement,
    ) -> EventRow;
}

/// This name is used to store the latest indexed block in the db.
pub(crate) const INDEX_NAME: &str = "onchain_orders";

#[async_trait::async_trait]
impl<T, W> EventStoring<(ContractEvent, Log)> for OnchainOrderParser<T, W>
where
    T: Send + Sync + Clone,
    W: Send + Sync + Clone,
{
    async fn last_event_block(&self) -> Result<u64> {
        let _timer = DatabaseMetrics::get()
            .database_queries
            .with_label_values(&["read_last_block_onchain_orders"])
            .start_timer();
        crate::boundary::events::read_last_block_from_db(&self.db.pool, INDEX_NAME).await
    }

    async fn persist_last_indexed_block(&mut self, latest_block: u64) -> Result<()> {
        let _timer = DatabaseMetrics::get()
            .database_queries
            .with_label_values(&["update_last_block_onchain_orders"])
            .start_timer();
        crate::boundary::events::write_last_block_to_db(&self.db.pool, latest_block, INDEX_NAME)
            .await
    }

    async fn replace_events(
        &mut self,
        events: Vec<(ContractEvent, Log)>,
        range: RangeInclusive<u64>,
    ) -> Result<()> {
        let _timer = DatabaseMetrics::get()
            .database_queries
            .with_label_values(&["replace_onchain_order_events"])
            .start_timer();

        let mut transaction = self.db.pool.begin().await?;

        self.delete_events(&mut transaction, range).await?;
        self.insert_events(events, &mut transaction).await?;

        transaction.commit().await.context("commit")?;

        Ok(())
    }

    async fn append_events(&mut self, events: Vec<(ContractEvent, Log)>) -> Result<()> {
        let _timer = DatabaseMetrics::get()
            .database_queries
            .with_label_values(&["append_onchain_order_events"])
            .start_timer();

        let mut transaction = self.db.pool.begin().await?;
        self.insert_events(events, &mut transaction).await?;
        transaction.commit().await.context("commit")?;

        Ok(())
    }
}

impl<T: Send + Sync + Clone, W: Send + Sync> OnchainOrderParser<T, W> {
    async fn extract_custom_and_general_order_data(
        &self,
        order_placement_events: Vec<(ContractEvent, Log)>,
    ) -> Result<(
        Vec<W>,
        Vec<Option<database::orders::Quote>>,
        Vec<(database::events::EventIndex, OnchainOrderPlacement)>,
        Vec<Order>,
        Vec<TxHash>,
    )> {
        let block_number_timestamp_hashmap =
            get_block_numbers_of_events(&self.web3, &order_placement_events).await?;
        let custom_event_data = self
            .custom_onchain_data_parser
            .parse_custom_event_data(&order_placement_events)?;
        let mut custom_data_hashmap = HashMap::new();
        let mut quote_id_hashmap = HashMap::new();
        for (event_index, custom_onchain_data) in custom_event_data.iter() {
            if let Some(additional_data) = &custom_onchain_data.additional_data {
                custom_data_hashmap.insert(*event_index, additional_data.clone());
            }
            quote_id_hashmap.insert(*event_index, custom_onchain_data.quote_id);
        }
        let events_and_quotes = order_placement_events
            .into_iter()
            .filter_map(|(event, log)| {
                let tx_hash = log.transaction_hash?;
                let event_index = log_to_event_index(&log)?;
                let quote_id = quote_id_hashmap.get(&event_index)?;

                Some((
                    event,
                    log,
                    // timestamp must be available, as otherwise, the
                    // function get_block_numbers_of_events would have errored
                    *block_number_timestamp_hashmap
                        .get(&(event_index.block_number as u64))
                        .unwrap() as i64,
                    *quote_id,
                    tx_hash,
                ))
            });
        let onchain_order_data = parse_general_onchain_order_placement_data(
            &*self.quoter,
            events_and_quotes,
            self.domain_separator,
            self.settlement_contract,
            self.metrics,
        )
        .await;

        let data_tuple = onchain_order_data.into_iter().map(
            |(event_index, quote, onchain_order_placement, order, tx_hash)| {
                (
                    self.custom_onchain_data_parser
                        .customized_event_data_for_event_index(
                            &event_index,
                            &order,
                            &custom_data_hashmap,
                            &onchain_order_placement,
                        ),
                    quote,
                    (event_index, onchain_order_placement),
                    order,
                    tx_hash,
                )
            },
        );
        Ok(multiunzip(data_tuple))
    }

    async fn delete_events(
        &self,
        transaction: &mut sqlx::Transaction<'static, sqlx::Postgres>,
        range: RangeInclusive<u64>,
    ) -> Result<()> {
        database::onchain_broadcasted_orders::mark_as_reorged(
            transaction,
            i64::try_from(*range.start()).unwrap_or(i64::MAX),
        )
        .await
        .context("mark_onchain_order_events failed")?;

        database::onchain_invalidations::delete_invalidations(
            transaction,
            i64::try_from(*range.start()).unwrap_or(i64::MAX),
        )
        .await
        .context("invalidating_onchain_order_events failed")?;

        Ok(())
    }

    async fn insert_events(
        &self,
        events: Vec<(ContractEvent, Log)>,
        transaction: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    ) -> Result<()> {
        let order_placement_events = events
            .clone()
            .into_iter()
            .filter(|(event, _)| matches!(event, ContractEvent::OrderPlacement(_)))
            .collect();
        let invalidation_events = get_invalidation_events(events)?;
        let invalided_order_uids = extract_invalidated_order_uids(invalidation_events)?;
        let (custom_onchain_data, quotes, broadcasted_order_data, orders, tx_hashes) = self
            .extract_custom_and_general_order_data(order_placement_events)
            .await?;

        database::onchain_invalidations::insert_onchain_invalidations(
            transaction,
            invalided_order_uids.as_slice(),
        )
        .await
        .context("insert_onchain_invalidations failed")?;

        self.custom_onchain_data_parser
            .append_custom_order_info_to_db(transaction, custom_onchain_data)
            .await
            .context("append_custom_onchain_orders failed")?;

        database::onchain_broadcasted_orders::append(
            transaction,
            broadcasted_order_data.as_slice(),
        )
        .await
        .context("append_onchain_orders failed")?;

        // We only need to insert quotes for orders that will be included in an
        // auction (they are needed to compute solver rewards). If placement
        // failed, then the quote is not needed.
        insert_quotes(
            transaction,
            quotes
                .clone()
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .as_slice(),
        )
        .await
        .context("appending quotes for onchain orders failed")?;

        insert_order_hooks(transaction, &orders, &self.trampoline)
            .await
            .context("failed to insert hooks")?;

        database::orders::insert_orders_and_ignore_conflicts(transaction, orders.as_slice())
            .await
            .context("insert_orders failed")?;

        for order in &invalided_order_uids {
            tracing::debug!(?order, "invalidated order");
        }
        for (order, quote, tx_hash) in izip!(orders, quotes, tx_hashes) {
            tracing::debug!(order =? order.uid, ?quote, onchain_transaction_hash = ?tx_hash, "order created");
        }

        Ok(())
    }
}

async fn get_block_numbers_of_events(
    web3: &Web3,
    events: &[(ContractEvent, Log)],
) -> Result<HashMap<u64, u32>> {
    let mut event_block_numbers: Vec<u64> = events
        .iter()
        .map(|(_, log)| {
            log.block_number
                .ok_or_else(|| anyhow!("event without metadata"))
        })
        .collect::<Result<Vec<u64>>>()?;
    event_block_numbers.dedup();
    let futures = event_block_numbers
        .into_iter()
        .map(|block_number| async move {
            let timestamp = timestamp_of_block_in_seconds(
                &web3.provider,
                BlockNumberOrTag::Number(block_number),
            )
            .await?;
            Ok((block_number, timestamp))
        });
    let block_number_timestamp_pair: Vec<anyhow::Result<(u64, u32)>> =
        stream::iter(futures).buffer_unordered(10).collect().await;
    block_number_timestamp_pair.into_iter().collect()
}

fn get_invalidation_events(
    events: Vec<(ContractEvent, Log)>,
) -> Result<Vec<(EventIndex, OrderInvalidation)>> {
    events
        .into_iter()
        .filter_map(|(data, log)| {
            let Some(event_index) = log_to_event_index(&log) else {
                return Some(Err(anyhow!("invalidation event without metadata")));
            };
            let data = match data {
                ContractEvent::OrderInvalidation(event) => event,
                _ => {
                    return None;
                }
            };
            Some(Ok((event_index, data)))
        })
        .collect()
}

fn extract_invalidated_order_uids(
    invalidations: Vec<(EventIndex, OrderInvalidation)>,
) -> Result<Vec<(EventIndex, database::OrderUid)>> {
    invalidations
        .into_iter()
        .map(|(event_index, invalidation)| {
            Ok((
                event_index,
                // The following conversion should not error, as the contract
                // enforces that the enough bytes are sent
                // If the error happens anyways, we want to stop indexing and
                // hence escalate the error
                bytes_to_order_uid(&invalidation.orderUid)?,
            ))
        })
        .collect()
}

type GeneralOnchainOrderPlacementData = (
    EventIndex,
    Option<database::orders::Quote>,
    OnchainOrderPlacement,
    Order,
    TxHash,
);
async fn parse_general_onchain_order_placement_data<I>(
    quoter: &'_ dyn OrderQuoting,
    order_placement_events_and_quotes_zipped: I,
    domain_separator: DomainSeparator,
    settlement_contract: Address,
    metrics: &'static Metrics,
) -> Vec<GeneralOnchainOrderPlacementData>
where
    I: Iterator<Item = (ContractEvent, Log, i64, i64, TxHash)>,
{
    let futures = order_placement_events_and_quotes_zipped.map(
        |(data, log, event_timestamp, quote_id, tx_hash)| async move {
            let Some(event_index) = log_to_event_index(&log) else {
                metrics.inc_onchain_order_errors("no_metadata");
                return Err(anyhow!("event without metadata"));
            };
            let event = match data {
                ContractEvent::OrderPlacement(event) => event,
                _ => {
                    unreachable!(
                        "Should not try to parse orders from events other than OrderPlacement"
                    )
                }
            };
            let detailed_order_data =
                extract_order_data_from_onchain_order_placement_event(&event, domain_separator);
            if detailed_order_data.is_err() {
                metrics.inc_onchain_order_errors("bad_parsing");
            }
            let (order_data, owner, signing_scheme, order_uid) = detailed_order_data?;

            let quote_result = get_quote(quoter, order_data, signing_scheme, &quote_id).await;
            let order_data = convert_onchain_order_placement(
                &event,
                event_timestamp,
                quote_result.clone(),
                order_data,
                signing_scheme,
                order_uid,
                owner,
                settlement_contract,
                metrics,
            );
            let quote = match quote_result {
                Ok(quote) => Some(database::orders::Quote {
                    order_uid: order_data.1.uid,
                    gas_amount: quote.data.fee_parameters.gas_amount,
                    gas_price: quote.data.fee_parameters.gas_price,
                    sell_token_price: quote.data.fee_parameters.sell_token_price,
                    sell_amount: u256_to_big_decimal(&quote.sell_amount),
                    buy_amount: u256_to_big_decimal(&quote.buy_amount),
                    solver: ByteArray(*quote.data.solver.0),
                    verified: quote.data.verified,
                    metadata: quote.data.metadata.try_into()?,
                }),
                Err(err) => {
                    let err_label = err.to_metrics_label();
                    tracing::debug!(
                        "Could not retrieve a quote for order {:?}: {err_label}",
                        order_data.1.uid,
                    );
                    metrics.inc_onchain_order_errors(err_label);
                    None
                }
            };
            Ok((event_index, quote, order_data.0, order_data.1, tx_hash))
        },
    );
    let onchain_order_placement_data: Vec<Result<GeneralOnchainOrderPlacementData>> =
        stream::iter(futures).buffer_unordered(10).collect().await;
    onchain_order_placement_data
        .into_iter()
        .filter_map(|data| match data {
            Err(err) => {
                tracing::debug!("Error while parsing onchain orders: {err:?}");
                None
            }
            Ok(data) => Some(data),
        })
        .collect()
}

async fn get_quote(
    quoter: &dyn OrderQuoting,
    order_data: OrderData,
    signing_scheme: SigningScheme,
    quote_id: &i64,
) -> Result<Quote, OnchainOrderPlacementError> {
    let quote_signing_scheme = convert_signing_scheme_into_quote_signing_scheme(
        signing_scheme,
        false,
        // Currently, only ethflow orders are indexed with this onchain
        // parser. For ethflow orders, we are okay to subsidize the
        // orders and allow them to set the verification limit to 0.
        // For general orders, this could result in a too big subsidy.
        0u64,
    )
    .map_err(|_| OnchainOrderPlacementError::Other)?;

    let parameters = QuoteSearchParameters {
        sell_token: order_data.sell_token,
        buy_token: order_data.buy_token,
        sell_amount: order_data.sell_amount,
        buy_amount: order_data.buy_amount,
        fee_amount: order_data.fee_amount,
        kind: order_data.kind,
        signing_scheme: quote_signing_scheme,
        additional_gas: 0,
        // Verified quotes always have prices that are at most as good as unverified quotes but can
        // be lower.
        // If the best quote we can find or compute on the fly for this order suggests a worse
        // price than the order was created with it will cause a `QuoteNotFound` or other error.
        // Orders indexed with errors are not eligible for automatic refunding.
        // Because we want to be generous with refunding EthFlow orders we therefore don't request a
        // verified quote here on purpose.
        verification: Default::default(),
    };

    get_quote_and_check_fee(
        quoter,
        &parameters.clone(),
        Some(*quote_id),
        Some(order_data.fee_amount),
    )
    .await
    .map_err(|err| match err {
        ValidationError::Partial(_) => OnchainOrderPlacementError::PreValidationError,
        ValidationError::NonZeroFee => OnchainOrderPlacementError::NonZeroFee,
        _ => OnchainOrderPlacementError::Other,
    })
}

#[expect(clippy::too_many_arguments)]
fn convert_onchain_order_placement(
    order_placement: &ContractOrderPlacement,
    event_timestamp: i64,
    quote: Result<Quote, OnchainOrderPlacementError>,
    order_data: OrderData,
    signing_scheme: SigningScheme,
    order_uid: OrderUid,
    owner: Address,
    settlement_contract: Address,
    metrics: &'static Metrics,
) -> (OnchainOrderPlacement, Order) {
    // eth flow orders are expected to be within the market price so they are
    // executed fast (we don't want to reserve the user's ETH for too long)
    if quote.as_ref().is_ok_and(|quote| {
        !order_data.within_market(QuoteAmounts {
            sell: quote.sell_amount,
            buy: quote.buy_amount,
            fee: quote.fee_amount,
        })
    }) {
        tracing::debug!(%order_uid, ?owner, "order is outside market price");
        metrics.inc_onchain_order_errors("outside_market_price");
    }

    let order = database::orders::Order {
        uid: ByteArray(order_uid.0),
        owner: ByteArray(owner.0.0),
        creation_timestamp: Utc.timestamp_opt(event_timestamp, 0).unwrap(),
        sell_token: ByteArray(order_data.sell_token.0.0),
        buy_token: ByteArray(order_data.buy_token.0.0),
        receiver: order_data.receiver.map(|addr| ByteArray(addr.0.0)),
        sell_amount: u256_to_big_decimal(&order_data.sell_amount),
        buy_amount: u256_to_big_decimal(&order_data.buy_amount),
        valid_to: order_data.valid_to as i64,
        app_data: ByteArray(order_data.app_data.0),
        fee_amount: u256_to_big_decimal(&order_data.fee_amount),
        kind: order_kind_into(order_data.kind),
        partially_fillable: order_data.partially_fillable,
        signature: order_placement.signature.data.to_vec(),
        signing_scheme: signing_scheme_into(signing_scheme),
        settlement_contract: ByteArray(settlement_contract.0.0),
        sell_token_balance: sell_token_source_into(order_data.sell_token_balance),
        buy_token_balance: buy_token_destination_into(order_data.buy_token_balance),
        cancellation_timestamp: None,
        class: match order_data.fee_amount.is_zero() {
            true => OrderClass::Limit,
            false => OrderClass::Market,
        },
    };
    let onchain_order_placement_event = OnchainOrderPlacement {
        order_uid: ByteArray(order_uid.0),
        sender: ByteArray(*order_placement.sender.0),
        placement_error: quote.err(),
    };
    (onchain_order_placement_event, order)
}

fn extract_order_data_from_onchain_order_placement_event(
    order_placement: &ContractOrderPlacement,
    domain_separator: DomainSeparator,
) -> Result<(OrderData, Address, SigningScheme, OrderUid)> {
    let (signing_scheme, owner) = match order_placement.signature.scheme {
        0 => (
            SigningScheme::Eip1271,
            Address::from_slice(&order_placement.signature.data),
        ),
        1 => (SigningScheme::PreSign, order_placement.sender),
        // Signatures can only be 0 and 1 by definition in the smart contrac:
        // https://github.com/cowprotocol/ethflowcontract/blob/main/src/\
        // interfaces/ICoWSwapOnchainOrders.sol#L10
        _ => bail!("unreachable state while parsing owner"),
    };

    let receiver = match order_placement.order.receiver {
        Address(bytes) if bytes.0 == [0u8; 20] => None,
        receiver => Some(receiver),
    };

    let order_data = OrderData {
        sell_token: order_placement.order.sellToken,
        buy_token: order_placement.order.buyToken,
        receiver,
        sell_amount: order_placement.order.sellAmount,
        buy_amount: order_placement.order.buyAmount,
        valid_to: order_placement.order.validTo,
        app_data: AppDataHash(order_placement.order.appData.0),
        fee_amount: order_placement.order.feeAmount,
        kind: OrderKind::from_contract_bytes(order_placement.order.kind.0)?,
        partially_fillable: order_placement.order.partiallyFillable,
        sell_token_balance: SellTokenSource::from_contract_bytes(
            order_placement.order.sellTokenBalance.0,
        )?,
        buy_token_balance: BuyTokenDestination::from_contract_bytes(
            order_placement.order.buyTokenBalance.0,
        )?,
    };
    let order_uid = order_data.uid(&domain_separator, owner);
    Ok((order_data, owner, signing_scheme, order_uid))
}

async fn insert_order_hooks(
    db: &mut PgConnection,
    orders: &[Order],
    trampoline: &HooksTrampoline::Instance,
) -> Result<()> {
    let mut interactions_to_insert = vec![];

    let execute_via_trampoline = |hooks: Vec<app_data::Hook>| {
        trampoline
            .execute(
                hooks
                    .into_iter()
                    .map(|hook| Hook {
                        target: hook.target,
                        callData: alloy::primitives::Bytes::from(hook.call_data.clone()),
                        gasLimit: U256::from(hook.gas_limit),
                    })
                    .collect(),
            )
            .calldata()
            .to_vec()
    };

    for order in orders {
        let appdata_json = database::app_data::fetch(db, &order.app_data)
            .await
            .context("failed to fetch appdata")?;
        let Some(appdata_json) = appdata_json else {
            tracing::debug!(order = ?order.uid, "appdata for order is unknown");
            continue;
        };
        let Ok(parsed) = app_data::parse(&appdata_json) else {
            tracing::debug!(appdata = %String::from_utf8_lossy(&appdata_json), "could not parse appdata");
            continue;
        };
        if parsed.hooks.pre.is_empty() && parsed.hooks.post.is_empty() {
            continue; // no additional interactions to index
        }

        let interactions_count = database::orders::next_free_interaction_indices(db, order.uid)
            .await
            .context("failed to fetch interaction count")?;

        if !parsed.hooks.pre.is_empty() {
            let interaction = database::orders::Interaction {
                target: ByteArray(trampoline.address().0.0),
                value: 0.into(),
                data: execute_via_trampoline(parsed.hooks.pre),
                index: interactions_count.next_pre_interaction_index,
                execution: database::orders::ExecutionTime::Pre,
            };
            interactions_to_insert.push((order.uid, interaction));
        }

        if !parsed.hooks.post.is_empty() {
            let interaction = database::orders::Interaction {
                target: ByteArray(trampoline.address().0.0),
                value: 0.into(),
                data: execute_via_trampoline(parsed.hooks.post),
                index: interactions_count.next_post_interaction_index,
                execution: database::orders::ExecutionTime::Post,
            };
            interactions_to_insert.push((order.uid, interaction));
        }
    }

    database::orders::insert_or_overwrite_interactions(db, &interactions_to_insert)
        .await
        .context("could not insert interactions for orders")
}

#[derive(prometheus_metric_storage::MetricStorage, Clone, Debug)]
#[metric(subsystem = "onchain_orders")]
struct Metrics {
    /// Keeps track of errors in picking up onchain orders.
    /// Note that an order might be created even if an error is encountered.
    #[metric(labels("error_type"))]
    onchain_order_errors: prometheus::IntCounterVec,
}

impl Metrics {
    fn get() -> &'static Self {
        Self::instance(observe::metrics::get_storage_registry())
            .expect("unexpected error getting metrics instance")
    }

    fn inc_onchain_order_errors(&self, error_label: &str) {
        self.onchain_order_errors
            .with_label_values(&[error_label])
            .inc();
    }
}

#[cfg(test)]
mod test {
    #![allow(non_snake_case)]

    use {
        super::*,
        alloy::primitives::U256,
        contracts::alloy::CoWSwapOnchainOrders,
        database::{byte_array::ByteArray, onchain_broadcasted_orders::OnchainOrderPlacement},
        ethrpc::Web3,
        model::{
            DomainSeparator,
            order::{BuyTokenDestination, OrderData, OrderKind, SellTokenSource},
            signature::SigningScheme,
        },
        number::conversions::u256_to_big_decimal,
        shared::{
            db_order_conversions::{
                buy_token_destination_into,
                order_kind_into,
                sell_token_source_into,
                signing_scheme_into,
            },
            fee::FeeParameters,
            order_quoting::{MockOrderQuoting, Quote, QuoteData},
        },
        sqlx::PgPool,
    };

    #[test]
    fn test_extract_order_data_from_onchain_order_placement_event() {
        let sender = Address::from([4; 20]);
        let owner = Address::from([6; 20]);

        let sellToken = Address::from([1; 20]);
        let buyToken = Address::from([2; 20]);
        let receiver = Address::from([3; 20]);
        let sellAmount = U256::from(10);
        let buyAmount = U256::from(11);
        let validTo = 1;
        let appData = [5u8; 32].into();
        let feeAmount = U256::from(12);
        let kind = OrderKind::SELL.into();
        let partiallyFillable = true;
        let sellTokenBalance = SellTokenSource::ERC20.into();
        let buyTokenBalance = BuyTokenDestination::ERC20.into();

        let order_placement = ContractOrderPlacement {
            sender: Address::from([4; 20]),
            order: CoWSwapOnchainOrders::GPv2Order::Data {
                sellToken,
                buyToken,
                receiver,
                sellAmount,
                buyAmount,
                validTo,
                appData,
                feeAmount,
                kind,
                partiallyFillable,
                sellTokenBalance,
                buyTokenBalance,
            },
            signature: CoWSwapOnchainOrders::ICoWSwapOnchainOrders::OnchainSignature {
                scheme: 0,
                data: owner.0.into(),
            },
            data: vec![0u8, 0u8, 1u8, 2u8, 0u8, 0u8, 1u8, 2u8, 0u8, 0u8, 1u8, 2u8].into(),
        };
        let domain_separator = DomainSeparator([7u8; 32]);
        let (order_data, owner, signing_scheme, order_uid) =
            extract_order_data_from_onchain_order_placement_event(
                &order_placement,
                domain_separator,
            )
            .unwrap();
        let expected_order_data = OrderData {
            sell_token: sellToken,
            buy_token: buyToken,
            receiver: Some(receiver),
            sell_amount: sellAmount,
            buy_amount: buyAmount,
            valid_to: validTo,
            app_data: AppDataHash(appData.0),
            fee_amount: feeAmount,
            kind: OrderKind::Sell,
            partially_fillable: order_placement.order.partiallyFillable,
            sell_token_balance: SellTokenSource::Erc20,
            buy_token_balance: BuyTokenDestination::Erc20,
        };
        let expected_uid = expected_order_data.uid(&domain_separator, owner);
        assert_eq!(sender, order_placement.sender);
        assert_eq!(signing_scheme, SigningScheme::Eip1271);
        assert_eq!(order_data, expected_order_data);
        assert_eq!(expected_uid, order_uid);

        let receiver = Address::ZERO;
        let order_placement = ContractOrderPlacement {
            sender: Address::from([4; 20]),
            order: CoWSwapOnchainOrders::GPv2Order::Data {
                sellToken,
                buyToken,
                receiver,
                sellAmount,
                buyAmount,
                validTo,
                appData,
                feeAmount,
                kind,
                partiallyFillable,
                sellTokenBalance,
                buyTokenBalance,
            },
            signature: CoWSwapOnchainOrders::ICoWSwapOnchainOrders::OnchainSignature {
                scheme: 1,
                data: owner.0.into(),
            },
            data: vec![0u8, 0u8, 1u8, 2u8, 0u8, 0u8, 1u8, 2u8, 0u8, 0u8, 1u8, 2u8].into(),
        };

        let (order_data, owner, signing_scheme, _) =
            extract_order_data_from_onchain_order_placement_event(
                &order_placement,
                domain_separator,
            )
            .unwrap();
        let expected_order_data = OrderData {
            sell_token: sellToken,
            buy_token: buyToken,
            receiver: None,
            sell_amount: sellAmount,
            buy_amount: buyAmount,
            valid_to: validTo,
            app_data: AppDataHash(appData.0),
            fee_amount: feeAmount,
            kind: OrderKind::Sell,
            partially_fillable: order_placement.order.partiallyFillable,
            sell_token_balance: SellTokenSource::Erc20,
            buy_token_balance: BuyTokenDestination::Erc20,
        };
        assert_eq!(order_data, expected_order_data);
        assert_eq!(signing_scheme, SigningScheme::PreSign);
        assert_eq!(owner, sender);
    }

    #[test]
    fn test_convert_onchain_order_placement() {
        let sell_token = Address::from([1; 20]);
        let buy_token = Address::from([2; 20]);
        let receiver = Address::from([3; 20]);
        let sender = Address::from([4; 20]);
        let sell_amount = U256::from(10);
        let buy_amount = U256::from(11);
        let valid_to = 1u32;
        let app_data = [11u8; 32];
        let fee_amount = U256::from(12);
        let owner = Address::from([5; 20]);

        let order_data = OrderData {
            sell_token,
            buy_token,
            receiver: Some(receiver),
            sell_amount,
            buy_amount,
            valid_to,
            app_data: AppDataHash(app_data),
            fee_amount,
            kind: OrderKind::Sell,
            partially_fillable: false,
            sell_token_balance: SellTokenSource::Erc20,
            buy_token_balance: BuyTokenDestination::Erc20,
        };

        let order_placement = ContractOrderPlacement {
            sender,
            order: contracts::alloy::CoWSwapOnchainOrders::GPv2Order::Data {
                sellToken: sell_token,
                buyToken: buy_token,
                receiver,
                sellAmount: sell_amount,
                buyAmount: buy_amount,
                validTo: valid_to,
                appData: app_data.into(),
                feeAmount: fee_amount,
                kind: OrderKind::SELL.into(),
                partiallyFillable: false,
                sellTokenBalance: SellTokenSource::ERC20.into(),
                buyTokenBalance: BuyTokenDestination::ERC20.into(),
            },
            signature:
                contracts::alloy::CoWSwapOnchainOrders::ICoWSwapOnchainOrders::OnchainSignature {
                    scheme: 0,
                    data: owner.0.into(),
                },
            data: Default::default(),
        };
        let settlement_contract = Address::repeat_byte(8);
        let quote = Quote::default();
        let order_uid = OrderUid([9u8; 56]);
        let signing_scheme = SigningScheme::Eip1271;
        let event_timestamp = 234354345;
        let (onchain_order_placement, order) = convert_onchain_order_placement(
            &order_placement,
            event_timestamp,
            Ok(quote),
            order_data,
            signing_scheme,
            order_uid,
            owner,
            settlement_contract,
            Metrics::get(),
        );
        let expected_order_data = OrderData {
            sell_token,
            buy_token,
            receiver: Some(receiver),
            sell_amount,
            buy_amount,
            valid_to,
            app_data: AppDataHash(app_data),
            fee_amount,
            kind: OrderKind::Sell,
            partially_fillable: order_placement.order.partiallyFillable,
            sell_token_balance: SellTokenSource::Erc20,
            buy_token_balance: BuyTokenDestination::Erc20,
        };
        let expected_onchain_order_placement = OnchainOrderPlacement {
            order_uid: ByteArray(order_uid.0),
            sender: ByteArray(order_placement.sender.0.0),
            placement_error: None,
        };
        let expected_order = database::orders::Order {
            uid: ByteArray(order_uid.0),
            owner: ByteArray(owner.0.0),
            creation_timestamp: order.creation_timestamp, /* Using the actual result to keep test
                                                           * simple */
            sell_token: ByteArray(expected_order_data.sell_token.0.0),
            buy_token: ByteArray(expected_order_data.buy_token.0.0),
            receiver: expected_order_data.receiver.map(|addr| ByteArray(addr.0.0)),
            sell_amount: u256_to_big_decimal(&expected_order_data.sell_amount),
            buy_amount: u256_to_big_decimal(&expected_order_data.buy_amount),
            valid_to: expected_order_data.valid_to as i64,
            app_data: ByteArray(expected_order_data.app_data.0),
            fee_amount: u256_to_big_decimal(&expected_order_data.fee_amount),
            kind: order_kind_into(expected_order_data.kind),
            class: OrderClass::Market,
            partially_fillable: expected_order_data.partially_fillable,
            signature: order_placement.signature.data.to_vec(),
            signing_scheme: signing_scheme_into(SigningScheme::Eip1271),
            settlement_contract: ByteArray(settlement_contract.0.into()),
            sell_token_balance: sell_token_source_into(expected_order_data.sell_token_balance),
            buy_token_balance: buy_token_destination_into(expected_order_data.buy_token_balance),
            cancellation_timestamp: None,
        };
        assert_eq!(onchain_order_placement, expected_onchain_order_placement);
        assert_eq!(order, expected_order);
    }

    #[test]
    fn test_convert_onchain_limit_order_placement() {
        let sell_token = Address::from([1; 20]);
        let buy_token = Address::from([2; 20]);
        let receiver = Address::from([3; 20]);
        let sender = Address::from([4; 20]);
        let sell_amount = U256::from(10);
        let buy_amount = U256::from(11);
        let valid_to = 1u32;
        let app_data = [11u8; 32];
        let fee_amount = U256::ZERO;
        let owner = Address::from([5; 20]);
        let order_data = OrderData {
            sell_token,
            buy_token,
            receiver: Some(receiver),
            sell_amount,
            buy_amount,
            valid_to,
            app_data: AppDataHash(app_data),
            fee_amount,
            kind: OrderKind::Sell,
            partially_fillable: false,
            sell_token_balance: SellTokenSource::Erc20,
            buy_token_balance: BuyTokenDestination::Erc20,
        };
        let order_placement = ContractOrderPlacement {
            sender,
            order: contracts::alloy::CoWSwapOnchainOrders::GPv2Order::Data {
                sellToken: sell_token,
                buyToken: buy_token,
                receiver,
                sellAmount: sell_amount,
                buyAmount: buy_amount,
                validTo: valid_to,
                appData: app_data.into(),
                feeAmount: fee_amount,
                kind: OrderKind::SELL.into(),
                partiallyFillable: false,
                sellTokenBalance: SellTokenSource::ERC20.into(),
                buyTokenBalance: BuyTokenDestination::ERC20.into(),
            },
            signature:
                contracts::alloy::CoWSwapOnchainOrders::ICoWSwapOnchainOrders::OnchainSignature {
                    scheme: 0,
                    data: owner.0.into(),
                },
            data: Default::default(),
        };
        let settlement_contract = Address::repeat_byte(8);
        let quote = Quote {
            sell_amount,
            buy_amount: buy_amount / U256::from(2),
            ..Default::default()
        };
        let order_uid = OrderUid([9u8; 56]);
        let signing_scheme = SigningScheme::Eip1271;
        let (onchain_order_placement, order) = convert_onchain_order_placement(
            &order_placement,
            345634,
            Ok(quote),
            order_data,
            signing_scheme,
            order_uid,
            owner,
            settlement_contract,
            Metrics::get(),
        );
        let expected_order_data = OrderData {
            sell_token,
            buy_token,
            receiver: Some(receiver),
            sell_amount,
            buy_amount,
            valid_to,
            app_data: AppDataHash(app_data),
            fee_amount: alloy::primitives::U256::ZERO,
            kind: OrderKind::Sell,
            partially_fillable: order_placement.order.partiallyFillable,
            sell_token_balance: SellTokenSource::Erc20,
            buy_token_balance: BuyTokenDestination::Erc20,
        };
        let expected_onchain_order_placement = OnchainOrderPlacement {
            order_uid: ByteArray(order_uid.0),
            sender: ByteArray(order_placement.sender.0.0),
            placement_error: None,
        };
        let expected_order = database::orders::Order {
            uid: ByteArray(order_uid.0),
            owner: ByteArray(owner.0.0),
            creation_timestamp: order.creation_timestamp, /* Using the actual result to keep test
                                                           * simple */
            sell_token: ByteArray(expected_order_data.sell_token.0.0),
            buy_token: ByteArray(expected_order_data.buy_token.0.0),
            receiver: expected_order_data.receiver.map(|addr| ByteArray(addr.0.0)),
            sell_amount: u256_to_big_decimal(&expected_order_data.sell_amount),
            buy_amount: u256_to_big_decimal(&expected_order_data.buy_amount),
            valid_to: expected_order_data.valid_to as i64,
            app_data: ByteArray(expected_order_data.app_data.0),
            fee_amount: u256_to_big_decimal(&fee_amount),
            kind: order_kind_into(expected_order_data.kind),
            class: OrderClass::Limit,
            partially_fillable: expected_order_data.partially_fillable,
            signature: order_placement.signature.data.to_vec(),
            signing_scheme: signing_scheme_into(SigningScheme::Eip1271),
            settlement_contract: ByteArray(settlement_contract.0.into()),
            sell_token_balance: sell_token_source_into(expected_order_data.sell_token_balance),
            buy_token_balance: buy_token_destination_into(expected_order_data.buy_token_balance),
            cancellation_timestamp: None,
        };
        assert_eq!(onchain_order_placement, expected_onchain_order_placement);
        assert_eq!(order, expected_order);
    }

    #[ignore]
    #[tokio::test]
    async fn extract_custom_and_general_order_data_matches_quotes_with_correct_events() {
        let sell_token = Address::from([1; 20]);
        let buy_token = Address::from([2; 20]);
        let receiver = Address::from([3; 20]);
        let sender = Address::from([4; 20]);
        let sell_amount = U256::from(10);
        let buy_amount = U256::from(11);
        let valid_to = 1u32;
        let app_data = [5u8; 32];
        let fee_amount = U256::from(12);
        let owner = Address::from([6; 20]);
        let order_placement = ContractOrderPlacement {
            sender,
            order: contracts::alloy::CoWSwapOnchainOrders::GPv2Order::Data {
                sellToken: sell_token,
                buyToken: buy_token,
                receiver,
                sellAmount: sell_amount,
                buyAmount: buy_amount,
                validTo: valid_to,
                appData: app_data.into(),
                feeAmount: fee_amount,
                kind: OrderKind::SELL.into(),
                partiallyFillable: true,
                sellTokenBalance: SellTokenSource::ERC20.into(),
                buyTokenBalance: BuyTokenDestination::ERC20.into(),
            },
            signature:
                contracts::alloy::CoWSwapOnchainOrders::ICoWSwapOnchainOrders::OnchainSignature {
                    scheme: 0,
                    data: owner.0.into(),
                },
            data: vec![0u8, 0u8, 1u8, 2u8, 0u8, 0u8, 1u8, 2u8, 0u8, 0u8, 1u8, 2u8].into(),
        };

        let log_1 = Log {
            block_number: Some(1),
            log_index: Some(0),
            ..Default::default()
        };
        let event_data_1 = ContractEvent::OrderPlacement(order_placement.clone());
        let log_2 = Log {
            block_number: Some(3),
            log_index: Some(0),
            ..Default::default()
        };
        let mut order_placement_2 = order_placement.clone();
        // With the following operation, we will create an invalid event data, and hence
        // the whole event parsing process will produce an error for this event.
        order_placement_2.data = vec![].into();
        let event_data_2 = ContractEvent::OrderPlacement(order_placement_2);
        let domain_separator = DomainSeparator([7u8; 32]);
        let mut order_quoter = MockOrderQuoting::new();
        let quote = Quote {
            id: Some(0i64),
            data: QuoteData {
                sell_token,
                buy_token,
                quoted_sell_amount: sell_amount.checked_sub(U256::from(1)).unwrap(),
                quoted_buy_amount: buy_amount.checked_sub(U256::from(1)).unwrap(),
                fee_parameters: FeeParameters {
                    gas_amount: 2.0f64,
                    gas_price: 3.0f64,
                    sell_token_price: 4.0f64,
                },
                ..Default::default()
            },
            sell_amount,
            buy_amount,
            fee_amount,
        };
        let cloned_quote = quote.clone();
        order_quoter
            .expect_find_quote()
            .returning(move |_, _| Ok(cloned_quote.clone()));
        let mut custom_onchain_order_parser = MockOnchainOrderParsing::<u8, u8>::new();
        custom_onchain_order_parser
            .expect_parse_custom_event_data()
            .returning(|_| {
                Ok(vec![(
                    EventIndex {
                        block_number: 1i64,
                        log_index: 0i64,
                    },
                    OnchainOrderCustomData {
                        quote_id: 0i64,
                        additional_data: Some(2u8),
                    },
                )])
            });
        custom_onchain_order_parser
            .expect_append_custom_order_info_to_db()
            .returning(|_, _| Ok(()));
        custom_onchain_order_parser
            .expect_customized_event_data_for_event_index()
            .returning(|_, _, _, _| 1u8);
        let web3 = Web3::new_from_env();
        let onchain_order_parser = OnchainOrderParser {
            db: Postgres {
                pool: PgPool::connect_lazy("postgresql://").unwrap(),
                config: Default::default(),
            },
            trampoline: HooksTrampoline::Instance::deployed(&web3.provider)
                .await
                .unwrap(),
            web3,
            quoter: Arc::new(order_quoter),
            custom_onchain_data_parser: Box::new(custom_onchain_order_parser),
            domain_separator,
            settlement_contract: Address::ZERO,
            metrics: Metrics::get(),
        };
        let result = onchain_order_parser
            .extract_custom_and_general_order_data(vec![
                (event_data_1, log_1),
                (event_data_2, log_2),
            ])
            .await
            .unwrap();
        let expected_order_data = OrderData {
            sell_token,
            buy_token,
            receiver: Some(receiver),
            sell_amount,
            buy_amount,
            valid_to,
            app_data: AppDataHash(app_data),
            fee_amount,
            kind: OrderKind::Sell,
            partially_fillable: order_placement.order.partiallyFillable,
            sell_token_balance: SellTokenSource::Erc20,
            buy_token_balance: BuyTokenDestination::Erc20,
        };
        let expected_uid = expected_order_data.uid(&domain_separator, owner);
        let expected_event_index = EventIndex {
            block_number: 1,
            log_index: 0,
        };
        let expected_quote = database::orders::Quote {
            order_uid: ByteArray(expected_uid.0),
            gas_amount: quote.data.fee_parameters.gas_amount,
            gas_price: quote.data.fee_parameters.gas_price,
            sell_token_price: quote.data.fee_parameters.sell_token_price,
            sell_amount: u256_to_big_decimal(&quote.sell_amount),
            buy_amount: u256_to_big_decimal(&quote.buy_amount),
            solver: ByteArray(*quote.data.solver.0),
            verified: quote.data.verified,
            metadata: quote.data.metadata.try_into().unwrap(),
        };
        assert_eq!(result.1, vec![Some(expected_quote)]);
        assert_eq!(
            result.2,
            vec![(
                expected_event_index,
                OnchainOrderPlacement {
                    order_uid: ByteArray(expected_uid.0),
                    sender: ByteArray(sender.0.0),
                    placement_error: None,
                },
            )]
        );
    }
}
