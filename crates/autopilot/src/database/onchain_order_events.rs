use super::{events::meta_to_event_index, Metrics, Postgres};
use anyhow::{anyhow, bail, Context, Result};
use contracts::cowswap_onchain_orders::{
    event_data::OrderPlacement as ContractOrderPlacement, Event as ContractEvent,
};
use database::{
    byte_array::ByteArray, events::EventIndex, onchain_broadcasted_orders::OnchainOrderPlacement,
    orders::Order, PgTransaction,
};
use ethcontract::{Event as EthContractEvent, H160};
use model::{
    order::OrderUid,
    order::{BuyTokenDestination, OrderKind, SellTokenSource},
    signature::SigningScheme,
    DomainSeparator,
    {app_id::AppId, order::OrderData},
};
use number_conversions::u256_to_big_decimal;
use shared::{
    db_order_conversions::{
        buy_token_destination_into, order_kind_into, sell_token_source_into, signing_scheme_into,
    },
    event_handling::EventStoring,
    order_quoting::{OrderQuoting, QuoteSearchParameters},
    order_validation::{
        convert_signing_scheme_into_quote_kind, get_quote_and_check_fee,
        is_order_outside_market_price,
    },
};
use std::collections::HashMap;
use unzip_n::unzip_n;

unzip_n!(pub 3);

pub struct OnchainOrderParser<'a, T: Send + Sync, W: Send + Sync> {
    db: Postgres,
    quoter: Box<dyn OrderQuoting>,
    custom_onchain_data_parser: Box<dyn CustomOnchainOrderParsing<'a, T, W>>,
    domain_separator: DomainSeparator,
    settlement_contract: H160,
}

impl<'a, T: Send + Sync, W: Send + Sync> OnchainOrderParser<'a, T, W> {
    pub fn new(
        db: Postgres,
        quoter: Box<dyn OrderQuoting>,
        custom_onchain_data_parser: Box<dyn CustomOnchainOrderParsing<'a, T, W>>,
        domain_separator: DomainSeparator,
        settlement_contract: H160,
    ) -> Self {
        OnchainOrderParser {
            db,
            quoter,
            custom_onchain_data_parser,
            domain_separator,
            settlement_contract,
        }
    }
}
pub struct CustomParsedOnchaninData<T> {
    quote_id: i64,
    additional_data: Option<T>,
}

#[async_trait::async_trait]
pub trait CustomOnchainOrderParsing<'a, T: Send + Sync + Clone, W: Send + Sync>:
    Send + Sync
{
    async fn append_custom_order_info_to_db(
        &self,
        ex: &mut PgTransaction<'a>,
        custom_onchain_data: Vec<W>,
    ) -> Result<()>;

    fn parse_custom_event_data(
        &self,
        contract_events: &[EthContractEvent<ContractEvent>],
    ) -> Result<Vec<(EventIndex, CustomParsedOnchaninData<T>)>>;

    fn customized_event_data_for_event_index(
        &self,
        event_index: &EventIndex,
        order: &Order,
        hashmap: &HashMap<EventIndex, T>,
        onchain_order_placement: &OnchainOrderPlacement,
    ) -> W;
}

#[async_trait::async_trait]
impl<T: Sync + Send + Clone, W: Sync + Send> EventStoring<ContractEvent>
    for OnchainOrderParser<'_, T, W>
{
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

    async fn append_events(&mut self, events: Vec<EthContractEvent<ContractEvent>>) -> Result<()> {
        let (custom_order_data, broadcasted_order_data, orders) =
            self.extract_custom_and_general_order_data(events).await?;

        let _timer = Metrics::get()
            .database_queries
            .with_label_values(&["append_ethflow_order_events"])
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

        database::orders::insert_orders(&mut transaction, orders.as_slice())
            .await
            .context("insert_orders failed")?;

        transaction.commit().await.context("commit")?;
        Ok(())
    }

    async fn replace_events(
        &mut self,
        events: Vec<EthContractEvent<ContractEvent>>,
        range: std::ops::RangeInclusive<shared::event_handling::BlockNumber>,
    ) -> Result<()> {
        let (custom_onchain_data, broadcasted_order_data, orders) =
            self.extract_custom_and_general_order_data(events).await?;

        let _timer = Metrics::get()
            .database_queries
            .with_label_values(&["replace_onchain_order_events"])
            .start_timer();

        let mut transaction = self.db.0.begin().await?;

        database::onchain_broadcasted_orders::mark_as_reorged(
            &mut transaction,
            range.start().to_u64() as i64,
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

        database::orders::insert_orders(&mut transaction, orders.as_slice())
            .await
            .context("insert_orders failed")?;
        transaction.commit().await.context("commit")?;
        Ok(())
    }
}

impl<T: Send + Sync + Clone, W: Send + Sync> OnchainOrderParser<'_, T, W> {
    async fn extract_custom_and_general_order_data(
        &self,
        events: Vec<EthContractEvent<ContractEvent>>,
    ) -> Result<(
        Vec<W>,
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
        )
        .await?;
        let data_trouple =
            onchain_order_data
                .into_iter()
                .map(|(event_index, onchain_order_placement, order)| {
                    (
                        self.custom_onchain_data_parser
                            .customized_event_data_for_event_index(
                                &event_index,
                                &order,
                                &custom_data_hashmap,
                                &onchain_order_placement,
                            ),
                        (event_index, onchain_order_placement),
                        order,
                    )
                });
        Ok(data_trouple.unzip_n_vec())
    }
}

async fn parse_general_onchain_order_placement_data(
    quoter: &dyn OrderQuoting,
    contract_events_and_quotes_zipped: Vec<(EthContractEvent<ContractEvent>, i64)>,
    domain_separator: DomainSeparator,
    settlement_contract: H160,
) -> Result<Vec<(EventIndex, OnchainOrderPlacement, Order)>> {
    let futures = contract_events_and_quotes_zipped.into_iter().map(
        |(EthContractEvent { data, meta }, quote_id)| async move {
            let meta = match meta {
                Some(meta) => meta,
                None => return Err(anyhow!("event without metadata")),
            };
            let ContractEvent::OrderPlacement(event) = data;
            let order_data = convert_onchain_order_placement(
                quoter,
                &event,
                quote_id,
                domain_separator,
                settlement_contract,
            )
            .await;
            Ok((meta_to_event_index(&meta), order_data?))
        },
    );
    let onchain_order_placement_data = futures::future::join_all(futures).await;
    onchain_order_placement_data
        .into_iter()
        .filter_map(|data| match data {
            Err(err) => {
                tracing::debug!("Error while parsing onchain orders: {:}", err);
                None
            }
            Ok((_, None)) => None,
            Ok((event, Some(order_data))) => Some(Ok((event, order_data.0, order_data.1))),
        })
        .collect::<Result<Vec<_>>>()
}

async fn convert_onchain_order_placement(
    quoter: &dyn OrderQuoting,
    order_placement: &ContractOrderPlacement,
    quote_id: i64,
    domain_separator: DomainSeparator,
    settlement_contract: H160,
) -> Result<Option<(OnchainOrderPlacement, Order)>> {
    let (order_data, owner, signing_scheme, order_uid) =
        extract_order_data_from_onchain_order_placement_event(order_placement, domain_separator)?;

    let quote_kind = convert_signing_scheme_into_quote_kind(signing_scheme, false)
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
        quote_kind,
    };
    let quote = get_quote_and_check_fee(
        quoter,
        &parameters.clone(),
        Some(quote_id as i64),
        order_data.fee_amount,
        model::quote::QuoteSigningScheme::Eip1271 {
            onchain_order: true,
        },
    )
    .await
    .map_err(|err| {
        anyhow!(
            "Error while fetching the quote {:?} error: {:?}",
            parameters,
            err
        )
    })?;

    let full_fee_amount = quote.data.fee_parameters.unsubsidized();

    // Orders that are placed and priced outside the market (i.e. buying
    // more than the market can pay or selling less than the market wants)
    // get flagged as liquidity orders. The reasoning is that these orders
    // are not intended to be filled immediately and so need to be treated
    // slightly differently by the protocol.
    let is_liquidity_order =
        if is_order_outside_market_price(&parameters.sell_amount, &parameters.buy_amount, &quote) {
            tracing::debug!(%order_uid, ?owner, "order being flagged as outside market price");
            true
        } else {
            false
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
        is_liquidity_order,
        cancellation_timestamp: None,
    };

    let onchain_order_placement_event = OnchainOrderPlacement {
        order_uid: ByteArray(order_uid.0),
        sender: ByteArray(order_placement.sender.0),
    };
    Ok(Some((onchain_order_placement_event, order)))
}

fn convert_signature_data_to_owner(data: Vec<u8>) -> Result<H160> {
    let owner = H160::from_slice(&data[..20]);
    Ok(owner)
}

fn extract_order_data_from_onchain_order_placement_event(
    order_placement: &ContractOrderPlacement,
    domain_separator: DomainSeparator,
) -> Result<(OrderData, H160, SigningScheme, OrderUid)> {
    let (signing_scheme, owner) = match order_placement.signature.0 {
        0 => (
            SigningScheme::Eip1271,
            convert_signature_data_to_owner(order_placement.signature.1 .0.clone())?,
        ),
        1 => (SigningScheme::PreSign, order_placement.sender),
        _ => bail!("unreachable state while parsing owner"),
    };

    let order_data = OrderData {
        sell_token: order_placement.order.0,
        buy_token: order_placement.order.1,
        receiver: Some(order_placement.order.2),
        sell_amount: order_placement.order.3,
        buy_amount: order_placement.order.4,
        valid_to: order_placement.order.5,
        app_data: AppId(order_placement.order.6 .0),
        fee_amount: order_placement.order.7,
        kind: OrderKind::try_from(order_placement.order.8 .0)?,
        partially_fillable: order_placement.order.9,
        sell_token_balance: SellTokenSource::try_from(order_placement.order.10 .0)?,
        buy_token_balance: BuyTokenDestination::try_from(order_placement.order.11 .0)?,
    };
    let order_uid = order_data.uid(&domain_separator, &owner);
    Ok((order_data, owner, signing_scheme, order_uid))
}

#[cfg(test)]
mod test {
    use super::*;
    use contracts::cowswap_onchain_orders::event_data::OrderPlacement as ContractOrderPlacement;
    use database::{byte_array::ByteArray, onchain_broadcasted_orders::OnchainOrderPlacement};
    use ethcontract::{Bytes, EventMetadata, H160, H256, U256};
    use mockall::predicate::{always, eq};
    use model::{
        app_id::AppId,
        order::{BuyTokenDestination, OrderData, OrderKind, SellTokenSource},
        signature::SigningScheme,
        DomainSeparator,
    };
    use number_conversions::u256_to_big_decimal;
    use shared::{
        db_order_conversions::{
            buy_token_destination_into, order_kind_into, sell_token_source_into,
            signing_scheme_into,
        },
        order_quoting::{FindQuoteError, MockOrderQuoting, Quote},
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
                Bytes(OrderData::KIND_SELL),
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
                Bytes(OrderData::KIND_SELL),
                true,
                Bytes(OrderData::BALANCE_ERC20),
                Bytes(OrderData::BALANCE_ERC20),
            ),
            signature: (1u8, Bytes(owner.as_ref().into())),
            ..Default::default()
        };
        let (_, owner, signing_scheme, _) = extract_order_data_from_onchain_order_placement_event(
            &order_placement,
            domain_separator,
        )
        .unwrap();
        assert_eq!(signing_scheme, SigningScheme::PreSign);
        assert_eq!(owner, sender);
    }

    #[tokio::test]
    async fn test_convert_onchain_order_placement() {
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
                Bytes(OrderData::KIND_SELL),
                true,
                Bytes(OrderData::BALANCE_ERC20),
                Bytes(OrderData::BALANCE_ERC20),
            ),
            signature: (0u8, Bytes(owner.as_ref().into())),
            ..Default::default()
        };
        let domain_separator = DomainSeparator([7u8; 32]);
        let settlement_contract = H160::from([8u8; 20]);
        let quote_id = 5;
        let mut order_quoter = MockOrderQuoting::new();
        order_quoter.expect_find_quote().returning({
            move |_, _| {
                Ok(Quote {
                    ..Default::default()
                })
            }
        });
        let (onchain_order_placement, order) = convert_onchain_order_placement(
            &order_quoter,
            &order_placement,
            quote_id,
            domain_separator,
            settlement_contract,
        )
        .await
        .unwrap()
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
        let expected_onchain_order_placement = OnchainOrderPlacement {
            order_uid: ByteArray(expected_uid.0),
            sender: ByteArray(order_placement.sender.0),
        };
        let expected_order = database::orders::Order {
            uid: ByteArray(expected_uid.0),
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
            partially_fillable: expected_order_data.partially_fillable,
            signature: order_placement.signature.1 .0.clone(),
            signing_scheme: signing_scheme_into(SigningScheme::Eip1271),
            settlement_contract: ByteArray(settlement_contract.0),
            sell_token_balance: sell_token_source_into(expected_order_data.sell_token_balance),
            buy_token_balance: buy_token_destination_into(expected_order_data.buy_token_balance),
            full_fee_amount: u256_to_big_decimal(&U256::zero()),
            is_liquidity_order: false,
            cancellation_timestamp: None,
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
                Bytes(OrderData::KIND_SELL),
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
                // todo: Implement default for EvetMetadata
                address: H160::zero(),
                block_hash: H256::zero(),
                block_number: 1,
                transaction_hash: H256::zero(),
                transaction_index: 0usize,
                log_index: 0usize,
                transaction_log_index: None,
                log_type: None,
            }),
        };
        let mut event_data_2 = event_data_1.clone();
        event_data_2.meta = Some(EventMetadata {
            // todo: Implement default for EvetMetadata
            address: H160::zero(),
            block_hash: H256::zero(),
            block_number: 2, // <-- different block number
            transaction_hash: H256::zero(),
            transaction_index: 0usize,
            log_index: 0usize,
            transaction_log_index: None,
            log_type: None,
        });
        let domain_separator = DomainSeparator([7u8; 32]);
        let settlement_contract = H160::from([8u8; 20]);
        let quote_id_1 = 5i64;
        let mut order_quoter = MockOrderQuoting::new();
        order_quoter
            .expect_find_quote()
            .with(eq(Some(quote_id_1)), always())
            .returning(move |_, _| Ok(Quote::default()));
        let quote_id_2 = 6i64;
        order_quoter
            .expect_find_quote()
            .with(eq(Some(quote_id_2)), always())
            .returning(move |_, _| Err(FindQuoteError::NotFound(None)));
        let result_vec = parse_general_onchain_order_placement_data(
            &order_quoter,
            vec![
                (event_data_1.clone(), quote_id_1),
                (event_data_2.clone(), quote_id_2),
            ],
            domain_separator,
            settlement_contract,
        )
        .await
        .unwrap();
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
}
