pub mod ethflow_events;

use super::{events::meta_to_event_index, Metrics, Postgres};
use anyhow::{anyhow, bail, Context, Result};
use contracts::cowswap_onchain_orders::{
    event_data::OrderPlacement as ContractOrderPlacement, Event as ContractEvent,
};
use database::{
    byte_array::ByteArray,
    events::EventIndex,
    onchain_broadcasted_orders::OnchainOrderPlacement,
    orders::{insert_quotes, Order, OrderClass},
    PgTransaction,
};
use ethcontract::{Event as EthContractEvent, H160};
use futures::{stream, StreamExt};
use itertools::multiunzip;
use model::{
    app_id::AppId,
    order::{BuyTokenDestination, OrderData, OrderKind, OrderUid, SellTokenSource},
    signature::SigningScheme,
    DomainSeparator,
};
use number_conversions::u256_to_big_decimal;
use shared::{
    current_block::RangeInclusive,
    db_order_conversions::{
        buy_token_destination_into, order_kind_into, sell_token_source_into, signing_scheme_into,
    },
    event_handling::EventStoring,
    order_quoting::{OrderQuoting, Quote, QuoteSearchParameters},
    order_validation::{
        convert_signing_scheme_into_quote_signing_scheme, get_quote_and_check_fee,
        is_order_outside_market_price,
    },
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

pub struct OnchainOrderParser<EventData: Send + Sync, EventRow: Send + Sync> {
    db: Postgres,
    quoter: Arc<dyn OrderQuoting>,
    custom_onchain_data_parser: Box<dyn OnchainOrderParsing<EventData, EventRow>>,
    domain_separator: DomainSeparator,
    settlement_contract: H160,
    liquidity_order_owners: HashSet<H160>,
}

impl<EventData, EventRow> OnchainOrderParser<EventData, EventRow>
where
    EventData: Send + Sync,
    EventRow: Send + Sync,
{
    pub fn new(
        db: Postgres,
        quoter: Arc<dyn OrderQuoting>,
        custom_onchain_data_parser: Box<dyn OnchainOrderParsing<EventData, EventRow>>,
        domain_separator: DomainSeparator,
        settlement_contract: H160,
        liquidity_order_owners: HashSet<H160>,
    ) -> Self {
        OnchainOrderParser {
            db,
            quoter,
            custom_onchain_data_parser,
            domain_separator,
            settlement_contract,
            liquidity_order_owners,
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

#[async_trait::async_trait]
impl<T: Sync + Send + Clone, W: Sync + Send + Clone> EventStoring<ContractEvent>
    for OnchainOrderParser<T, W>
{
    async fn replace_events(
        &mut self,
        events: Vec<EthContractEvent<ContractEvent>>,
        range: RangeInclusive<u64>,
    ) -> Result<()> {
        let order_placement_events = events
            .into_iter()
            .filter(|EthContractEvent { data, .. }| {
                matches!(data, ContractEvent::OrderPlacement(_))
            })
            .collect();
        let (custom_onchain_data, quotes, broadcasted_order_data, orders) = self
            .extract_custom_and_general_order_data(order_placement_events)
            .await?;

        let _timer = Metrics::get()
            .database_queries
            .with_label_values(&["replace_onchain_order_events"])
            .start_timer();

        let mut transaction = self.db.0.begin().await?;

        database::onchain_broadcasted_orders::mark_as_reorged(
            &mut transaction,
            *range.start() as i64,
        )
        .await
        .context("mark_onchain_order_events failed")?;

        self.custom_onchain_data_parser
            .append_custom_order_info_to_db(&mut transaction, custom_onchain_data)
            .await
            .context("append_custom_onchain_orders failed")?;

        database::onchain_broadcasted_orders::append(
            &mut transaction,
            broadcasted_order_data.as_slice(),
        )
        .await
        .context("append_onchain_orders failed")?;

        insert_quotes(&mut transaction, quotes.as_slice())
            .await
            .context("appending quotes for onchain orders failed")?;

        database::orders::insert_orders_and_ignore_conflicts(&mut transaction, orders.as_slice())
            .await
            .context("insert_orders failed")?;
        transaction.commit().await.context("commit")?;
        Ok(())
    }

    async fn append_events(&mut self, events: Vec<EthContractEvent<ContractEvent>>) -> Result<()> {
        let order_placement_events = events
            .into_iter()
            .filter(|EthContractEvent { data, .. }| {
                matches!(data, ContractEvent::OrderPlacement(_))
            })
            .collect();
        let (custom_order_data, quotes, broadcasted_order_data, orders) = self
            .extract_custom_and_general_order_data(order_placement_events)
            .await?;

        let _timer = Metrics::get()
            .database_queries
            .with_label_values(&["append_onchain_order_events"])
            .start_timer();
        let mut transaction = self.db.0.begin().await?;

        database::onchain_broadcasted_orders::append(
            &mut transaction,
            broadcasted_order_data.as_slice(),
        )
        .await
        .context("append_onchain_orders failed")?;

        self.custom_onchain_data_parser
            .append_custom_order_info_to_db(&mut transaction, custom_order_data)
            .await
            .context("append_custom_onchain_orders failed")?;

        insert_quotes(&mut transaction, quotes.as_slice())
            .await
            .context("appending quotes for onchain orders failed")?;

        database::orders::insert_orders_and_ignore_conflicts(&mut transaction, orders.as_slice())
            .await
            .context("insert_orders failed")?;

        transaction.commit().await.context("commit")?;
        Ok(())
    }

    async fn last_event_block(&self) -> Result<u64> {
        let _timer = Metrics::get()
            .database_queries
            .with_label_values(&["last_event_block"])
            .start_timer();

        let mut con = self.db.0.acquire().await?;
        let block_number = database::onchain_broadcasted_orders::last_block(&mut con)
            .await
            .context("block_number_of_most_recent_event failed")?;
        block_number.try_into().context("block number is negative")
    }
}

impl<T: Send + Sync + Clone, W: Send + Sync> OnchainOrderParser<T, W> {
    async fn extract_custom_and_general_order_data(
        &self,
        events: Vec<EthContractEvent<ContractEvent>>,
    ) -> Result<(
        Vec<W>,
        Vec<database::orders::Quote>,
        Vec<(database::events::EventIndex, OnchainOrderPlacement)>,
        Vec<Order>,
    )> {
        let custom_event_data = self
            .custom_onchain_data_parser
            .parse_custom_event_data(&events)?;
        let mut custom_data_hashmap = HashMap::new();
        let mut quote_id_hashmap = HashMap::new();
        for (event_index, custom_onchain_data) in custom_event_data.iter() {
            if let Some(additional_data) = &custom_onchain_data.additional_data {
                custom_data_hashmap.insert(*event_index, additional_data.clone());
            }
            quote_id_hashmap.insert(*event_index, custom_onchain_data.quote_id);
        }
        let mut events_and_quotes = Vec::new();
        for event in events.iter() {
            let EthContractEvent { meta, .. } = event;
            if let Some(meta) = meta {
                let event_index = meta_to_event_index(meta);
                if let Some(quote_id) = quote_id_hashmap.get(&event_index) {
                    events_and_quotes.push((event.clone(), *quote_id));
                }
            }
        }
        let onchain_order_data = parse_general_onchain_order_placement_data(
            &*self.quoter,
            events_and_quotes,
            self.domain_separator,
            self.settlement_contract,
            &self.liquidity_order_owners,
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
}

type GeneralOnchainOrderPlacementData = (
    EventIndex,
    database::orders::Quote,
    OnchainOrderPlacement,
    Order,
);
async fn parse_general_onchain_order_placement_data(
    quoter: &dyn OrderQuoting,
    contract_events_and_quotes_zipped: Vec<(EthContractEvent<ContractEvent>, i64)>,
    domain_separator: DomainSeparator,
    settlement_contract: H160,
    liquidity_order_owners: &HashSet<H160>,
) -> Vec<GeneralOnchainOrderPlacementData> {
    let futures = contract_events_and_quotes_zipped.into_iter().map(
        |(EthContractEvent { data, meta }, quote_id)| async move {
            let meta = match meta {
                Some(meta) => meta,
                None => return Err(anyhow!("event without metadata")),
            };
            let event = match data {
                ContractEvent::OrderPlacement(event) => event,
                _ => {
                    return Err(anyhow!(
                        "parse_general_onchain_order_placement_data should not reach this state"
                    ))
                }
            };
            let (order_data, owner, signing_scheme, order_uid) =
                extract_order_data_from_onchain_order_placement_event(&event, domain_separator)?;

            let quote = get_quote(quoter, order_data, signing_scheme, &event, &quote_id).await?;
            let order_data = convert_onchain_order_placement(
                &event,
                quote.clone(),
                order_data,
                signing_scheme,
                order_uid,
                owner,
                settlement_contract,
                liquidity_order_owners,
            )?;
            let quote = database::orders::Quote {
                order_uid: order_data.1.uid,
                gas_amount: quote.data.fee_parameters.gas_amount,
                gas_price: quote.data.fee_parameters.gas_price,
                sell_token_price: quote.data.fee_parameters.sell_token_price,
                sell_amount: u256_to_big_decimal(&quote.sell_amount),
                buy_amount: u256_to_big_decimal(&quote.buy_amount),
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
                tracing::debug!("Error while parsing onchain orders: {:}", err);
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
    order_placement: &ContractOrderPlacement,
    quote_id: &i64,
) -> Result<Quote> {
    let quote_signing_scheme = convert_signing_scheme_into_quote_signing_scheme(
        signing_scheme,
        false,
        // Currently, only ethflow orders are indexed with this onchain
        // parser. For ethflow orders, we are okay to subsidize the
        // orders and allow them to set the verification limit to 0.
        // For general orders, this could result in a too big subsidy.
        0u64,
    )
    .map_err(|err| anyhow!("Error invalid signature transformation: {:?}", err))?;

    let parameters = QuoteSearchParameters {
        sell_token: H160::from(order_data.sell_token.0),
        buy_token: H160::from(order_data.buy_token.0),
        sell_amount: order_data.sell_amount,
        buy_amount: order_data.buy_amount,
        fee_amount: order_data.fee_amount,
        kind: order_data.kind,
        // Original quote was made from user account, and not necessarily from owner.
        from: order_placement.sender,
        app_data: order_data.app_data,
    };
    get_quote_and_check_fee(
        quoter,
        &parameters.clone(),
        Some(*quote_id as i64),
        order_data.fee_amount,
        quote_signing_scheme,
    )
    .await
    .map_err(|err| {
        anyhow!(
            "Error while fetching the quote {:?} error: {:?}",
            parameters,
            err
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn convert_onchain_order_placement(
    order_placement: &ContractOrderPlacement,
    quote: Quote,
    order_data: OrderData,
    signing_scheme: SigningScheme,
    order_uid: OrderUid,
    owner: H160,
    settlement_contract: H160,
    liquidity_order_owners: &HashSet<H160>,
) -> Result<(OnchainOrderPlacement, Order)> {
    let full_fee_amount = quote.data.fee_parameters.unsubsidized();

    let is_outside_market_price =
        if is_order_outside_market_price(&order_data.sell_amount, &order_data.buy_amount, &quote) {
            tracing::debug!(%order_uid, ?owner, "order being flagged as outside market price");
            true
        } else {
            false
        };

    let liquidity_owner = if liquidity_order_owners.contains(&owner) {
        tracing::debug!(%order_uid, ?owner, "order being flagged as placed by liquidity order owner");
        true
    } else {
        false
    };

    // TODO(nlordell): It is currently possible to create limit orders from
    // on-chain events even if they are disabled at the API level. This feels
    // non-intentional, and we should revisit this before releasing EthFlow
    // orders.
    let class = match (is_outside_market_price, liquidity_owner) {
        (true, true) => OrderClass::Liquidity,
        (true, false) => OrderClass::Limit,
        _ => OrderClass::Market,
    };

    let order = database::orders::Order {
        uid: ByteArray(order_uid.0),
        owner: ByteArray(owner.0),
        creation_timestamp: chrono::offset::Utc::now(),
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
        full_fee_amount: u256_to_big_decimal(&full_fee_amount),
        cancellation_timestamp: None,
        class,
        surplus_fee: Default::default(),
        surplus_fee_timestamp: Default::default(),
    };
    let onchain_order_placement_event = OnchainOrderPlacement {
        order_uid: ByteArray(order_uid.0),
        sender: ByteArray(order_placement.sender.0),
    };
    Ok((onchain_order_placement_event, order))
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
        app_data: AppId(order_placement.order.6 .0),
        fee_amount: order_placement.order.7,
        kind: OrderKind::from_contract_bytes(order_placement.order.8 .0)?,
        partially_fillable: order_placement.order.9,
        sell_token_balance: SellTokenSource::from_contract_bytes(order_placement.order.10 .0)?,
        buy_token_balance: BuyTokenDestination::from_contract_bytes(order_placement.order.11 .0)?,
    };
    let order_uid = order_data.uid(&domain_separator, &owner);
    Ok((order_data, owner, signing_scheme, order_uid))
}

#[cfg(test)]
mod test {
    use super::*;
    use contracts::cowswap_onchain_orders::event_data::OrderPlacement as ContractOrderPlacement;
    use database::{byte_array::ByteArray, onchain_broadcasted_orders::OnchainOrderPlacement};
    use ethcontract::{Bytes, EventMetadata, H160, U256};
    use maplit::hashset;
    use mockall::predicate::{always, eq};
    use model::{
        app_id::AppId,
        order::{BuyTokenDestination, OrderData, OrderKind, SellTokenSource},
        quote::QuoteSigningScheme,
        signature::SigningScheme,
        DomainSeparator,
    };
    use number_conversions::u256_to_big_decimal;
    use shared::{
        db_order_conversions::{
            buy_token_destination_into, order_kind_into, sell_token_source_into,
            signing_scheme_into,
        },
        fee_subsidy::FeeParameters,
        order_quoting::{FindQuoteError, MockOrderQuoting, Quote, QuoteData},
    };
    use sqlx::PgPool;

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
                Bytes(OrderData::BALANCE_ERC20),
                Bytes(OrderData::BALANCE_ERC20),
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
            app_data: AppId(app_data.0),
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
                Bytes(OrderData::BALANCE_ERC20),
                Bytes(OrderData::BALANCE_ERC20),
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
            app_data: AppId(app_data.0),
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
            app_data: AppId(app_data.0),
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
                Bytes(OrderData::BALANCE_ERC20),
                Bytes(OrderData::BALANCE_ERC20),
            ),
            signature: (0u8, Bytes(owner.as_ref().into())),
            ..Default::default()
        };
        let settlement_contract = H160::from([8u8; 20]);
        let quote = Quote::default();
        let order_uid = OrderUid([9u8; 56]);
        let signing_scheme = SigningScheme::Eip1271;
        let (onchain_order_placement, order) = convert_onchain_order_placement(
            &order_placement,
            quote,
            order_data,
            signing_scheme,
            order_uid,
            owner,
            settlement_contract,
            &Default::default(),
        )
        .unwrap();
        let expected_order_data = OrderData {
            sell_token,
            buy_token,
            receiver: Some(receiver),
            sell_amount,
            buy_amount,
            valid_to,
            app_data: AppId(app_data.0),
            fee_amount,
            kind: OrderKind::Sell,
            partially_fillable: order_placement.order.9,
            sell_token_balance: SellTokenSource::Erc20,
            buy_token_balance: BuyTokenDestination::Erc20,
        };
        let expected_onchain_order_placement = OnchainOrderPlacement {
            order_uid: ByteArray(order_uid.0),
            sender: ByteArray(order_placement.sender.0),
        };
        let expected_order = database::orders::Order {
            uid: ByteArray(order_uid.0),
            owner: ByteArray(owner.0),
            creation_timestamp: order.creation_timestamp, // Using the actual result to keep test simple
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
            full_fee_amount: u256_to_big_decimal(&U256::zero()),
            cancellation_timestamp: None,
            ..Default::default()
        };
        assert_eq!(onchain_order_placement, expected_onchain_order_placement);
        assert_eq!(order, expected_order);
    }

    #[test]
    fn test_convert_onchain_liquidity_order_placement() {
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
            app_data: AppId(app_data.0),
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
                Bytes(OrderData::BALANCE_ERC20),
                Bytes(OrderData::BALANCE_ERC20),
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
            quote,
            order_data,
            signing_scheme,
            order_uid,
            owner,
            settlement_contract,
            &hashset! {owner},
        )
        .unwrap();
        let expected_order_data = OrderData {
            sell_token,
            buy_token,
            receiver: Some(receiver),
            sell_amount,
            buy_amount,
            valid_to,
            app_data: AppId(app_data.0),
            fee_amount,
            kind: OrderKind::Sell,
            partially_fillable: order_placement.order.9,
            sell_token_balance: SellTokenSource::Erc20,
            buy_token_balance: BuyTokenDestination::Erc20,
        };
        let expected_onchain_order_placement = OnchainOrderPlacement {
            order_uid: ByteArray(order_uid.0),
            sender: ByteArray(order_placement.sender.0),
        };
        let expected_order = database::orders::Order {
            uid: ByteArray(order_uid.0),
            owner: ByteArray(owner.0),
            creation_timestamp: order.creation_timestamp, // Using the actual result to keep test simple
            sell_token: ByteArray(expected_order_data.sell_token.0),
            buy_token: ByteArray(expected_order_data.buy_token.0),
            receiver: expected_order_data.receiver.map(|h160| ByteArray(h160.0)),
            sell_amount: u256_to_big_decimal(&expected_order_data.sell_amount),
            buy_amount: u256_to_big_decimal(&expected_order_data.buy_amount),
            valid_to: expected_order_data.valid_to as i64,
            app_data: ByteArray(expected_order_data.app_data.0),
            fee_amount: u256_to_big_decimal(&expected_order_data.fee_amount),
            kind: order_kind_into(expected_order_data.kind),
            class: OrderClass::Liquidity,
            partially_fillable: expected_order_data.partially_fillable,
            signature: order_placement.signature.1 .0,
            signing_scheme: signing_scheme_into(SigningScheme::Eip1271),
            settlement_contract: ByteArray(settlement_contract.0),
            sell_token_balance: sell_token_source_into(expected_order_data.sell_token_balance),
            buy_token_balance: buy_token_destination_into(expected_order_data.buy_token_balance),
            full_fee_amount: u256_to_big_decimal(&U256::zero()),
            cancellation_timestamp: None,
            ..Default::default()
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
        let fee_amount = U256::from_dec_str("12").unwrap();
        let owner = H160::from([5; 20]);
        let order_data = OrderData {
            sell_token,
            buy_token,
            receiver: Some(receiver),
            sell_amount,
            buy_amount,
            valid_to,
            app_data: AppId(app_data.0),
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
                Bytes(OrderData::BALANCE_ERC20),
                Bytes(OrderData::BALANCE_ERC20),
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
            quote,
            order_data,
            signing_scheme,
            order_uid,
            owner,
            settlement_contract,
            &Default::default(),
        )
        .unwrap();
        let expected_order_data = OrderData {
            sell_token,
            buy_token,
            receiver: Some(receiver),
            sell_amount,
            buy_amount,
            valid_to,
            app_data: AppId(app_data.0),
            fee_amount: 0.into(),
            kind: OrderKind::Sell,
            partially_fillable: order_placement.order.9,
            sell_token_balance: SellTokenSource::Erc20,
            buy_token_balance: BuyTokenDestination::Erc20,
        };
        let expected_onchain_order_placement = OnchainOrderPlacement {
            order_uid: ByteArray(order_uid.0),
            sender: ByteArray(order_placement.sender.0),
        };
        let expected_order = database::orders::Order {
            uid: ByteArray(order_uid.0),
            owner: ByteArray(owner.0),
            creation_timestamp: order.creation_timestamp, // Using the actual result to keep test simple
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
            surplus_fee: Default::default(),
            ..Default::default()
        };
        assert_eq!(onchain_order_placement, expected_onchain_order_placement);
        assert_eq!(order, expected_order);
    }

    #[tokio::test]
    async fn parse_general_onchain_order_placement_data_filters_out_errored_quotes() {
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
                Bytes(OrderData::BALANCE_ERC20),
                Bytes(OrderData::BALANCE_ERC20),
            ),
            signature: (0u8, Bytes(owner.as_ref().into())),
            data: ethcontract::Bytes(vec![
                0u8, 0u8, 1u8, 2u8, 0u8, 0u8, 1u8, 2u8, 0u8, 0u8, 1u8, 2u8,
            ]),
        };

        let signing_scheme = QuoteSigningScheme::Eip1271 {
            onchain_order: true,
            verification_gas_limit: 0u64,
        };

        let event_data_1 = EthContractEvent {
            data: ContractEvent::OrderPlacement(order_placement.clone()),
            meta: Some(EventMetadata {
                block_number: 1,
                log_index: 0usize,
                ..Default::default()
            }),
        };
        let mut event_data_2 = event_data_1.clone();
        event_data_2.meta = Some(EventMetadata {
            block_number: 2, // <-- different block number
            log_index: 0usize,
            ..Default::default()
        });
        let domain_separator = DomainSeparator([7u8; 32]);
        let settlement_contract = H160::from([8u8; 20]);
        let quote_id_1 = 5i64;
        let mut order_quoter = MockOrderQuoting::new();
        order_quoter
            .expect_find_quote()
            .with(eq(Some(quote_id_1)), always(), eq(signing_scheme))
            .with(eq(Some(quote_id_1)), always(), eq(signing_scheme))
            .returning(move |_, _, _| Ok(Quote::default()));
        let quote_id_2 = 6i64;
        order_quoter
            .expect_find_quote()
            .with(eq(Some(quote_id_2)), always(), eq(signing_scheme))
            .returning(move |_, _, _| Err(FindQuoteError::NotFound(None)));
        let result_vec = parse_general_onchain_order_placement_data(
            &order_quoter,
            vec![
                (event_data_1.clone(), quote_id_1),
                (event_data_2.clone(), quote_id_2),
            ],
            domain_separator,
            settlement_contract,
            &Default::default(),
        )
        .await;
        assert_eq!(result_vec.len(), 1);
        let first_element = result_vec.get(0).unwrap();
        assert_eq!(
            first_element.0,
            EventIndex {
                block_number: 1,
                log_index: 0i64
            }
        );
    }
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
                Bytes(OrderData::BALANCE_ERC20),
                Bytes(OrderData::BALANCE_ERC20),
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
        // With the following operation, we will create an invalid event data, and hence the whole
        // event parsing process will produce an error for this event.
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
            .returning(move |_, _, _| Ok(cloned_quote.clone()));
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
        let onchain_order_parser = OnchainOrderParser {
            db: Postgres(PgPool::connect_lazy("postgresql://").unwrap()),
            quoter: Arc::new(order_quoter),
            custom_onchain_data_parser: Box::new(custom_onchain_order_parser),
            domain_separator,
            settlement_contract: H160::zero(),
            liquidity_order_owners: Default::default(),
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
            app_data: AppId(app_data.0),
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
        };
        assert_eq!(result.1, vec![expected_quote]);
        assert_eq!(
            result.2,
            vec![(
                expected_event_index,
                OnchainOrderPlacement {
                    order_uid: ByteArray(expected_uid.0),
                    sender: ByteArray(sender.0),
                },
            )]
        );
    }
}
