pub mod ethflow_events;
pub mod event_retriever;

use {
    super::{
        events::{bytes_to_order_uid, meta_to_event_index},
        Metrics as DatabaseMetrics,
        Postgres,
    },
    anyhow::{anyhow, bail, Context, Result},
    app_data::AppDataHash,
    chrono::{TimeZone, Utc},
    contracts::cowswap_onchain_orders::{
        event_data::{OrderInvalidation, OrderPlacement as ContractOrderPlacement},
        Event as ContractEvent,
    },
    database::{
        byte_array::ByteArray,
        events::EventIndex,
        onchain_broadcasted_orders::{OnchainOrderPlacement, OnchainOrderPlacementError},
        orders::{insert_quotes, Order, OrderClass},
        PgTransaction,
    },
    ethcontract::{Event as EthContractEvent, H160},
    ethrpc::{
        block_stream::{timestamp_of_block_in_seconds, RangeInclusive},
        Web3,
    },
    futures::{stream, StreamExt},
    itertools::multiunzip,
    model::{
        order::{
            BuyTokenDestination,
            OrderData,
            OrderKind,
            OrderUid,
            QuoteAmounts,
            SellTokenSource,
        },
        signature::SigningScheme,
        DomainSeparator,
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
        order_quoting::{OrderQuoting, Quote, QuoteMetadata, QuoteSearchParameters},
        order_validation::{
            convert_signing_scheme_into_quote_signing_scheme,
            get_quote_and_check_fee,
            ValidationError,
        },
    },
    std::{collections::HashMap, sync::Arc},
    web3::types::U64,
};

pub struct OnchainOrderParser<EventData: Send + Sync, EventRow: Send + Sync> {
    db: Postgres,
    web3: Web3,
    quoter: Arc<dyn OrderQuoting>,
    custom_onchain_data_parser: Box<dyn OnchainOrderParsing<EventData, EventRow>>,
    domain_separator: DomainSeparator,
    settlement_contract: H160,
    metrics: &'static Metrics,
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
        settlement_contract: H160,
    ) -> Self {
        OnchainOrderParser {
            db,
            web3,
            quoter,
            custom_onchain_data_parser,
            domain_separator,
            settlement_contract,
            metrics: Metrics::get(),
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
        contract_events: &[EthContractEvent<ContractEvent>],
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
const INDEX_NAME: &str = "onchain_orders";

#[async_trait::async_trait]
impl<T: Sync + Send + Clone, W: Sync + Send + Clone> EventStoring<ContractEvent>
    for OnchainOrderParser<T, W>
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
        events: Vec<EthContractEvent<ContractEvent>>,
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

    async fn append_events(&mut self, events: Vec<EthContractEvent<ContractEvent>>) -> Result<()> {
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
        order_placement_events: Vec<EthContractEvent<ContractEvent>>,
    ) -> Result<(
        Vec<W>,
        Vec<Option<database::orders::Quote>>,
        Vec<(database::events::EventIndex, OnchainOrderPlacement)>,
        Vec<Order>,
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
        let mut events_and_quotes = Vec::new();
        for event in order_placement_events.iter() {
            let EthContractEvent { meta, .. } = event;
            if let Some(meta) = meta {
                let event_index = meta_to_event_index(meta);
                if let Some(quote_id) = quote_id_hashmap.get(&event_index) {
                    events_and_quotes.push((
                        event.clone(),
                        // timestamp must be available, as otherwise, the
                        // function get_block_numbers_of_events would have errored
                        *block_number_timestamp_hashmap
                            .get(&(event_index.block_number as u64))
                            .unwrap() as i64,
                        *quote_id,
                    ));
                }
            }
        }
        let onchain_order_data = parse_general_onchain_order_placement_data(
            &*self.quoter,
            events_and_quotes,
            self.domain_separator,
            self.settlement_contract,
            self.metrics,
        )
        .await;

        let data_tuple = onchain_order_data.into_iter().map(
            |(event_index, quote, onchain_order_placement, order)| {
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
        events: Vec<EthContractEvent<ContractEvent>>,
        transaction: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    ) -> Result<()> {
        let order_placement_events = events
            .clone()
            .into_iter()
            .filter(|EthContractEvent { data, .. }| {
                matches!(data, ContractEvent::OrderPlacement(_))
            })
            .collect();
        let invalidation_events = get_invalidation_events(events)?;
        let invalided_order_uids = extract_invalidated_order_uids(invalidation_events)?;
        let (custom_onchain_data, quotes, broadcasted_order_data, orders) = self
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

        database::orders::insert_orders_and_ignore_conflicts(transaction, orders.as_slice())
            .await
            .context("insert_orders failed")?;

        for order in &invalided_order_uids {
            tracing::debug!(?order, "invalidated order");
        }
        for (order, quote) in orders.iter().zip(quotes.iter()) {
            tracing::debug!(order =? order.uid, ?quote, "order created");
        }

        Ok(())
    }
}

async fn get_block_numbers_of_events(
    web3: &Web3,
    events: &[EthContractEvent<ContractEvent>],
) -> Result<HashMap<u64, u32>> {
    let mut event_block_numbers: Vec<u64> = events
        .iter()
        .map(|EthContractEvent { meta, .. }| {
            let meta = match meta {
                Some(meta) => meta,
                None => return Err(anyhow!("event without metadata")),
            };
            Ok(meta.block_number)
        })
        .collect::<Result<Vec<u64>>>()?;
    event_block_numbers.dedup();
    let futures = event_block_numbers
        .into_iter()
        .map(|block_number| async move {
            let timestamp =
                timestamp_of_block_in_seconds(web3, U64::from(block_number).into()).await?;
            Ok((block_number, timestamp))
        });
    let block_number_timestamp_pair: Vec<anyhow::Result<(u64, u32)>> =
        stream::iter(futures).buffer_unordered(10).collect().await;
    block_number_timestamp_pair.into_iter().collect()
}

fn get_invalidation_events(
    events: Vec<EthContractEvent<ContractEvent>>,
) -> Result<Vec<(EventIndex, OrderInvalidation)>> {
    events
        .into_iter()
        .filter_map(|EthContractEvent { data, meta }| {
            let meta = match meta {
                Some(meta) => meta,
                None => return Some(Err(anyhow!("invalidation event without metadata"))),
            };
            let data = match data {
                ContractEvent::OrderInvalidation(event) => event,
                _ => {
                    return None;
                }
            };
            Some(Ok((meta_to_event_index(&meta), data)))
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
                bytes_to_order_uid(invalidation.order_uid.0.as_slice())?,
            ))
        })
        .collect()
}

type GeneralOnchainOrderPlacementData = (
    EventIndex,
    Option<database::orders::Quote>,
    OnchainOrderPlacement,
    Order,
);
async fn parse_general_onchain_order_placement_data<'a>(
    quoter: &'a dyn OrderQuoting,
    order_placement_events_and_quotes_zipped: Vec<(EthContractEvent<ContractEvent>, i64, i64)>,
    domain_separator: DomainSeparator,
    settlement_contract: H160,
    metrics: &'static Metrics,
) -> Vec<GeneralOnchainOrderPlacementData> {
    let futures = order_placement_events_and_quotes_zipped.into_iter().map(
        |(EthContractEvent { data, meta }, event_timestamp, quote_id)| async move {
            let meta = match meta {
                Some(meta) => meta,
                None => {
                    metrics.inc_onchain_order_errors("no_metadata");
                    return Err(anyhow!("event without metadata"));
                }
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
                    solver: ByteArray(quote.data.solver.0),
                    verified: Some(quote.data.verified),
                    metadata: Some(
                        QuoteMetadata {
                            interactions: quote.data.interactions.clone(),
                        }
                        .try_into()?,
                    ),
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
            Ok((
                meta_to_event_index(&meta),
                quote,
                order_data.0,
                order_data.1,
            ))
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
        sell_token: H160::from(order_data.sell_token.0),
        buy_token: H160::from(order_data.buy_token.0),
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

#[allow(clippy::too_many_arguments)]
fn convert_onchain_order_placement(
    order_placement: &ContractOrderPlacement,
    event_timestamp: i64,
    quote: Result<Quote, OnchainOrderPlacementError>,
    order_data: OrderData,
    signing_scheme: SigningScheme,
    order_uid: OrderUid,
    owner: H160,
    settlement_contract: H160,
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
        owner: ByteArray(owner.0),
        creation_timestamp: Utc.timestamp_opt(event_timestamp, 0).unwrap(),
        sell_token: ByteArray(order_data.sell_token.0),
        buy_token: ByteArray(order_data.buy_token.0),
        receiver: order_data.receiver.map(|h160| ByteArray(h160.0)),
        sell_amount: u256_to_big_decimal(&order_data.sell_amount),
        buy_amount: u256_to_big_decimal(&order_data.buy_amount),
        valid_to: order_data.valid_to as i64,
        app_data: ByteArray(order_data.app_data.0),
        fee_amount: u256_to_big_decimal(&order_data.fee_amount),
        kind: order_kind_into(order_data.kind),
        partially_fillable: order_data.partially_fillable,
        signature: order_placement.signature.1 .0.clone(),
        signing_scheme: signing_scheme_into(signing_scheme),
        settlement_contract: ByteArray(settlement_contract.0),
        sell_token_balance: sell_token_source_into(order_data.sell_token_balance),
        buy_token_balance: buy_token_destination_into(order_data.buy_token_balance),
        full_fee_amount: u256_to_big_decimal(&order_data.fee_amount),
        cancellation_timestamp: None,
        class: match order_data.fee_amount.is_zero() {
            true => OrderClass::Limit,
            false => OrderClass::Market,
        },
    };
    let onchain_order_placement_event = OnchainOrderPlacement {
        order_uid: ByteArray(order_uid.0),
        sender: ByteArray(order_placement.sender.0),
        placement_error: quote.err(),
    };
    (onchain_order_placement_event, order)
}

fn extract_order_data_from_onchain_order_placement_event(
    order_placement: &ContractOrderPlacement,
    domain_separator: DomainSeparator,
) -> Result<(OrderData, H160, SigningScheme, OrderUid)> {
    let (signing_scheme, owner) = match order_placement.signature.0 {
        0 => (
            SigningScheme::Eip1271,
            H160::from_slice(&order_placement.signature.1 .0[..20]),
        ),
        1 => (SigningScheme::PreSign, order_placement.sender),
        // Signatures can only be 0 and 1 by definition in the smart contrac:
        // https://github.com/cowprotocol/ethflowcontract/blob/main/src/\
        // interfaces/ICoWSwapOnchainOrders.sol#L10
        _ => bail!("unreachable state while parsing owner"),
    };

    let receiver = match order_placement.order.2 {
        H160(bytes) if bytes == [0u8; 20] => None,
        receiver => Some(receiver),
    };

    let order_data = OrderData {
        sell_token: order_placement.order.0,
        buy_token: order_placement.order.1,
        receiver,
        sell_amount: order_placement.order.3,
        buy_amount: order_placement.order.4,
        valid_to: order_placement.order.5,
        app_data: AppDataHash(order_placement.order.6 .0),
        fee_amount: order_placement.order.7,
        kind: OrderKind::from_contract_bytes(order_placement.order.8 .0)?,
        partially_fillable: order_placement.order.9,
        sell_token_balance: SellTokenSource::from_contract_bytes(order_placement.order.10 .0)?,
        buy_token_balance: BuyTokenDestination::from_contract_bytes(order_placement.order.11 .0)?,
    };
    let order_uid = order_data.uid(&domain_separator, &owner);
    Ok((order_data, owner, signing_scheme, order_uid))
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
    use {
        super::*,
        crate::database::Config,
        contracts::cowswap_onchain_orders::event_data::OrderPlacement as ContractOrderPlacement,
        database::{byte_array::ByteArray, onchain_broadcasted_orders::OnchainOrderPlacement},
        ethcontract::{Bytes, EventMetadata, H160, U256},
        model::{
            order::{BuyTokenDestination, OrderData, OrderKind, SellTokenSource},
            signature::SigningScheme,
            DomainSeparator,
        },
        number::conversions::u256_to_big_decimal,
        shared::{
            db_order_conversions::{
                buy_token_destination_into,
                order_kind_into,
                sell_token_source_into,
                signing_scheme_into,
            },
            ethrpc::create_env_test_transport,
            fee::FeeParameters,
            order_quoting::{MockOrderQuoting, Quote, QuoteData},
        },
        sqlx::PgPool,
        std::num::NonZeroUsize,
    };

    #[test]
    fn test_extract_order_data_from_onchain_order_placement_event() {
        let sell_token = H160::from([1; 20]);
        let buy_token = H160::from([2; 20]);
        let receiver = H160::from([3; 20]);
        let sender = H160::from([4; 20]);
        let sell_amount = U256::from_dec_str("10").unwrap();
        let buy_amount = U256::from_dec_str("11").unwrap();
        let valid_to = 1u32;
        let app_data = ethcontract::tokens::Bytes([5u8; 32]);
        let fee_amount = U256::from_dec_str("12").unwrap();
        let owner = H160::from([6; 20]);
        let order_placement = ContractOrderPlacement {
            sender,
            order: (
                sell_token,
                buy_token,
                receiver,
                sell_amount,
                buy_amount,
                valid_to,
                app_data,
                fee_amount,
                Bytes(OrderKind::SELL),
                true,
                Bytes(SellTokenSource::ERC20),
                Bytes(BuyTokenDestination::ERC20),
            ),
            signature: (0u8, Bytes(owner.as_ref().into())),
            ..Default::default()
        };
        let domain_separator = DomainSeparator([7u8; 32]);
        let (order_data, owner, signing_scheme, order_uid) =
            extract_order_data_from_onchain_order_placement_event(
                &order_placement,
                domain_separator,
            )
            .unwrap();
        let expected_order_data = OrderData {
            sell_token,
            buy_token,
            receiver: Some(receiver),
            sell_amount,
            buy_amount,
            valid_to,
            app_data: AppDataHash(app_data.0),
            fee_amount,
            kind: OrderKind::Sell,
            partially_fillable: order_placement.order.9,
            sell_token_balance: SellTokenSource::Erc20,
            buy_token_balance: BuyTokenDestination::Erc20,
        };
        let expected_uid = expected_order_data.uid(&domain_separator, &owner);
        assert_eq!(sender, order_placement.sender);
        assert_eq!(signing_scheme, SigningScheme::Eip1271);
        assert_eq!(order_data, expected_order_data);
        assert_eq!(expected_uid, order_uid);

        let receiver = H160::zero();
        let order_placement = ContractOrderPlacement {
            sender,
            order: (
                sell_token,
                buy_token,
                receiver,
                sell_amount,
                buy_amount,
                valid_to,
                app_data,
                fee_amount,
                Bytes(OrderKind::SELL),
                true,
                Bytes(SellTokenSource::ERC20),
                Bytes(BuyTokenDestination::ERC20),
            ),
            signature: (1u8, Bytes(owner.as_ref().into())),
            ..Default::default()
        };
        let (order_data, owner, signing_scheme, _) =
            extract_order_data_from_onchain_order_placement_event(
                &order_placement,
                domain_separator,
            )
            .unwrap();
        let expected_order_data = OrderData {
            sell_token,
            buy_token,
            receiver: None,
            sell_amount,
            buy_amount,
            valid_to,
            app_data: AppDataHash(app_data.0),
            fee_amount,
            kind: OrderKind::Sell,
            partially_fillable: order_placement.order.9,
            sell_token_balance: SellTokenSource::Erc20,
            buy_token_balance: BuyTokenDestination::Erc20,
        };
        assert_eq!(order_data, expected_order_data);
        assert_eq!(signing_scheme, SigningScheme::PreSign);
        assert_eq!(owner, sender);
    }

    #[test]
    fn test_convert_onchain_order_placement() {
        let sell_token = H160::from([1; 20]);
        let buy_token = H160::from([2; 20]);
        let receiver = H160::from([3; 20]);
        let sender = H160::from([4; 20]);
        let sell_amount = U256::from_dec_str("10").unwrap();
        let buy_amount = U256::from_dec_str("11").unwrap();
        let valid_to = 1u32;
        let app_data = ethcontract::tokens::Bytes([11u8; 32]);
        let fee_amount = U256::from_dec_str("12").unwrap();
        let owner = H160::from([5; 20]);
        let order_data = OrderData {
            sell_token,
            buy_token,
            receiver: Some(receiver),
            sell_amount,
            buy_amount,
            valid_to,
            app_data: AppDataHash(app_data.0),
            fee_amount,
            kind: OrderKind::Sell,
            partially_fillable: false,
            sell_token_balance: SellTokenSource::Erc20,
            buy_token_balance: BuyTokenDestination::Erc20,
        };
        let order_placement = ContractOrderPlacement {
            sender,
            order: (
                sell_token,
                buy_token,
                receiver,
                sell_amount,
                buy_amount,
                valid_to,
                app_data,
                fee_amount,
                Bytes(OrderKind::SELL),
                false,
                Bytes(SellTokenSource::ERC20),
                Bytes(BuyTokenDestination::ERC20),
            ),
            signature: (0u8, Bytes(owner.as_ref().into())),
            ..Default::default()
        };
        let settlement_contract = H160::from([8u8; 20]);
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
            app_data: AppDataHash(app_data.0),
            fee_amount,
            kind: OrderKind::Sell,
            partially_fillable: order_placement.order.9,
            sell_token_balance: SellTokenSource::Erc20,
            buy_token_balance: BuyTokenDestination::Erc20,
        };
        let expected_onchain_order_placement = OnchainOrderPlacement {
            order_uid: ByteArray(order_uid.0),
            sender: ByteArray(order_placement.sender.0),
            placement_error: None,
        };
        let expected_order = database::orders::Order {
            uid: ByteArray(order_uid.0),
            owner: ByteArray(owner.0),
            creation_timestamp: order.creation_timestamp, /* Using the actual result to keep test
                                                           * simple */
            sell_token: ByteArray(expected_order_data.sell_token.0),
            buy_token: ByteArray(expected_order_data.buy_token.0),
            receiver: expected_order_data.receiver.map(|h160| ByteArray(h160.0)),
            sell_amount: u256_to_big_decimal(&expected_order_data.sell_amount),
            buy_amount: u256_to_big_decimal(&expected_order_data.buy_amount),
            valid_to: expected_order_data.valid_to as i64,
            app_data: ByteArray(expected_order_data.app_data.0),
            fee_amount: u256_to_big_decimal(&expected_order_data.fee_amount),
            kind: order_kind_into(expected_order_data.kind),
            class: OrderClass::Market,
            partially_fillable: expected_order_data.partially_fillable,
            signature: order_placement.signature.1 .0,
            signing_scheme: signing_scheme_into(SigningScheme::Eip1271),
            settlement_contract: ByteArray(settlement_contract.0),
            sell_token_balance: sell_token_source_into(expected_order_data.sell_token_balance),
            buy_token_balance: buy_token_destination_into(expected_order_data.buy_token_balance),
            full_fee_amount: u256_to_big_decimal(&expected_order_data.fee_amount),
            cancellation_timestamp: None,
        };
        assert_eq!(onchain_order_placement, expected_onchain_order_placement);
        assert_eq!(order, expected_order);
    }

    #[test]
    fn test_convert_onchain_limit_order_placement() {
        let sell_token = H160::from([1; 20]);
        let buy_token = H160::from([2; 20]);
        let receiver = H160::from([3; 20]);
        let sender = H160::from([4; 20]);
        let sell_amount = U256::from_dec_str("10").unwrap();
        let buy_amount = U256::from_dec_str("11").unwrap();
        let valid_to = 1u32;
        let app_data = ethcontract::tokens::Bytes([11u8; 32]);
        let fee_amount = U256::from_dec_str("0").unwrap();
        let owner = H160::from([5; 20]);
        let order_data = OrderData {
            sell_token,
            buy_token,
            receiver: Some(receiver),
            sell_amount,
            buy_amount,
            valid_to,
            app_data: AppDataHash(app_data.0),
            fee_amount,
            kind: OrderKind::Sell,
            partially_fillable: false,
            sell_token_balance: SellTokenSource::Erc20,
            buy_token_balance: BuyTokenDestination::Erc20,
        };
        let order_placement = ContractOrderPlacement {
            sender,
            order: (
                sell_token,
                buy_token,
                receiver,
                sell_amount,
                buy_amount,
                valid_to,
                app_data,
                fee_amount,
                Bytes(OrderKind::SELL),
                false,
                Bytes(SellTokenSource::ERC20),
                Bytes(BuyTokenDestination::ERC20),
            ),
            signature: (0u8, Bytes(owner.as_ref().into())),
            ..Default::default()
        };
        let settlement_contract = H160::from([8u8; 20]);
        let quote = Quote {
            sell_amount,
            buy_amount: buy_amount / 2,
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
            app_data: AppDataHash(app_data.0),
            fee_amount: 0.into(),
            kind: OrderKind::Sell,
            partially_fillable: order_placement.order.9,
            sell_token_balance: SellTokenSource::Erc20,
            buy_token_balance: BuyTokenDestination::Erc20,
        };
        let expected_onchain_order_placement = OnchainOrderPlacement {
            order_uid: ByteArray(order_uid.0),
            sender: ByteArray(order_placement.sender.0),
            placement_error: None,
        };
        let expected_order = database::orders::Order {
            uid: ByteArray(order_uid.0),
            owner: ByteArray(owner.0),
            creation_timestamp: order.creation_timestamp, /* Using the actual result to keep test
                                                           * simple */
            sell_token: ByteArray(expected_order_data.sell_token.0),
            buy_token: ByteArray(expected_order_data.buy_token.0),
            receiver: expected_order_data.receiver.map(|h160| ByteArray(h160.0)),
            sell_amount: u256_to_big_decimal(&expected_order_data.sell_amount),
            buy_amount: u256_to_big_decimal(&expected_order_data.buy_amount),
            valid_to: expected_order_data.valid_to as i64,
            app_data: ByteArray(expected_order_data.app_data.0),
            fee_amount: u256_to_big_decimal(&fee_amount),
            kind: order_kind_into(expected_order_data.kind),
            class: OrderClass::Limit,
            partially_fillable: expected_order_data.partially_fillable,
            signature: order_placement.signature.1 .0,
            signing_scheme: signing_scheme_into(SigningScheme::Eip1271),
            settlement_contract: ByteArray(settlement_contract.0),
            sell_token_balance: sell_token_source_into(expected_order_data.sell_token_balance),
            buy_token_balance: buy_token_destination_into(expected_order_data.buy_token_balance),
            full_fee_amount: u256_to_big_decimal(&U256::zero()),
            cancellation_timestamp: None,
        };
        assert_eq!(onchain_order_placement, expected_onchain_order_placement);
        assert_eq!(order, expected_order);
    }

    #[ignore]
    #[tokio::test]
    async fn extract_custom_and_general_order_data_matches_quotes_with_correct_events() {
        let sell_token = H160::from([1; 20]);
        let buy_token = H160::from([2; 20]);
        let receiver = H160::from([3; 20]);
        let sender = H160::from([4; 20]);
        let sell_amount = U256::from_dec_str("10").unwrap();
        let buy_amount = U256::from_dec_str("11").unwrap();
        let valid_to = 1u32;
        let app_data = ethcontract::tokens::Bytes([5u8; 32]);
        let fee_amount = U256::from_dec_str("12").unwrap();
        let owner = H160::from([6; 20]);
        let order_placement = ContractOrderPlacement {
            sender,
            order: (
                sell_token,
                buy_token,
                receiver,
                sell_amount,
                buy_amount,
                valid_to,
                app_data,
                fee_amount,
                Bytes(OrderKind::SELL),
                true,
                Bytes(SellTokenSource::ERC20),
                Bytes(BuyTokenDestination::ERC20),
            ),
            signature: (0u8, Bytes(owner.as_ref().into())),
            data: ethcontract::Bytes(vec![
                0u8, 0u8, 1u8, 2u8, 0u8, 0u8, 1u8, 2u8, 0u8, 0u8, 1u8, 2u8,
            ]),
        };

        let event_data_1 = EthContractEvent {
            data: ContractEvent::OrderPlacement(order_placement.clone()),
            meta: Some(EventMetadata {
                block_number: 1,
                log_index: 0usize,
                ..Default::default()
            }),
        };
        let mut order_placement_2 = order_placement.clone();
        // With the following operation, we will create an invalid event data, and hence
        // the whole event parsing process will produce an error for this event.
        order_placement_2.data = Bytes(Vec::new());
        let event_data_2 = EthContractEvent {
            data: ContractEvent::OrderPlacement(order_placement_2),
            meta: Some(EventMetadata {
                block_number: 3,
                log_index: 0usize,
                ..Default::default()
            }),
        };
        let domain_separator = DomainSeparator([7u8; 32]);
        let mut order_quoter = MockOrderQuoting::new();
        let quote = Quote {
            id: Some(0i64),
            data: QuoteData {
                sell_token,
                buy_token,
                quoted_sell_amount: sell_amount.checked_sub(1.into()).unwrap(),
                quoted_buy_amount: buy_amount.checked_sub(1.into()).unwrap(),
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
        let web3 = Web3::new(create_env_test_transport());
        let onchain_order_parser = OnchainOrderParser {
            db: Postgres {
                pool: PgPool::connect_lazy("postgresql://").unwrap(),
                config: Config {
                    insert_batch_size: NonZeroUsize::new(500).unwrap(),
                },
            },
            web3,
            quoter: Arc::new(order_quoter),
            custom_onchain_data_parser: Box::new(custom_onchain_order_parser),
            domain_separator,
            settlement_contract: H160::zero(),
            metrics: Metrics::get(),
        };
        let result = onchain_order_parser
            .extract_custom_and_general_order_data(vec![event_data_1.clone(), event_data_2.clone()])
            .await
            .unwrap();
        let expected_order_data = OrderData {
            sell_token,
            buy_token,
            receiver: Some(receiver),
            sell_amount,
            buy_amount,
            valid_to,
            app_data: AppDataHash(app_data.0),
            fee_amount,
            kind: OrderKind::Sell,
            partially_fillable: order_placement.order.9,
            sell_token_balance: SellTokenSource::Erc20,
            buy_token_balance: BuyTokenDestination::Erc20,
        };
        let expected_uid = expected_order_data.uid(&domain_separator, &owner);
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
            solver: ByteArray(quote.data.solver.0),
            verified: Some(quote.data.verified),
            metadata: Some(
                QuoteMetadata {
                    interactions: quote.data.interactions,
                }
                .try_into()
                .unwrap(),
            ),
        };
        assert_eq!(result.1, vec![Some(expected_quote)]);
        assert_eq!(
            result.2,
            vec![(
                expected_event_index,
                OnchainOrderPlacement {
                    order_uid: ByteArray(expected_uid.0),
                    sender: ByteArray(sender.0),
                    placement_error: None,
                },
            )]
        );
    }
}
