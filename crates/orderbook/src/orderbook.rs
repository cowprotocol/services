use {
    crate::{
        database::{
            orders::{InsertionError, OrderStoring, OrderWithQuote},
            trades::{TradeFilter, TradeRetrieving},
        },
        dto,
        solver_competition::{Identifier, SolverCompetitionStoring},
    },
    anyhow::{Context, Result},
    app_data::{AppDataHash, Validator},
    chrono::Utc,
    database::order_events::OrderEventLabel,
    ethcontract::H256,
    model::{
        order::{
            Order,
            OrderCancellation,
            OrderCreation,
            OrderCreationAppData,
            OrderStatus,
            OrderUid,
            SignedOrderCancellations,
        },
        quote::QuoteId,
        solver_competition::{self, SolverCompetitionAPI},
        DomainSeparator,
    },
    number::conversions::big_decimal_to_u256,
    observe::metrics::LivenessChecking,
    primitive_types::H160,
    shared::{
        fee::FeeParameters,
        order_quoting::Quote,
        order_validation::{
            is_order_outside_market_price,
            Amounts,
            OrderValidating,
            ValidationError,
        },
    },
    std::{borrow::Cow, sync::Arc},
    strum_macros::Display,
    thiserror::Error,
};

#[derive(prometheus_metric_storage::MetricStorage, Clone, Debug)]
#[metric(subsystem = "orderbook")]
struct Metrics {
    /// Counter for measuring order statistics.
    #[metric(labels("kind", "operation"))]
    orders: prometheus::IntCounterVec,
}

#[derive(Display)]
#[strum(serialize_all = "snake_case")]
enum OrderOperation {
    Created,
    Cancelled,
}

#[derive(Display)]
#[strum(serialize_all = "snake_case")]
enum OrderClass {
    Market,
    Limit,
}

impl Metrics {
    fn get() -> &'static Self {
        Self::instance(observe::metrics::get_storage_registry())
            .expect("unexpected error getting metrics instance")
    }

    fn on_order_operation(order: &OrderWithQuote, operation: OrderOperation) {
        let class = if order.quote.as_ref().is_some_and(|quote| {
            // Check if the order at the submission time was "in market"
            !is_order_outside_market_price(
                &Amounts {
                    sell: order.order.data.sell_amount,
                    buy: order.order.data.buy_amount,
                    fee: order.order.data.fee_amount,
                },
                &Amounts {
                    sell: big_decimal_to_u256(&quote.sell_amount).unwrap(),
                    buy: big_decimal_to_u256(&quote.buy_amount).unwrap(),
                    fee: FeeParameters {
                        gas_amount: quote.gas_amount,
                        gas_price: quote.gas_price,
                        sell_token_price: quote.sell_token_price,
                    }
                    .fee(),
                },
                order.order.data.kind,
            )
        }) {
            OrderClass::Market
        } else {
            OrderClass::Limit
        };
        Self::get()
            .orders
            .with_label_values(&[&class.to_string(), &operation.to_string()])
            .inc();
    }

    // Resets all the counters to 0 so we can always use them in Grafana queries.
    fn initialize() {
        let metrics = Self::get();
        for op in &[OrderOperation::Created, OrderOperation::Cancelled] {
            for class in &[OrderClass::Market, OrderClass::Limit] {
                metrics
                    .orders
                    .with_label_values(&[&class.to_string(), &op.to_string()])
                    .reset();
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum AddOrderError {
    #[error("unable to find an existing order: {0}")]
    OrderNotFound(#[source] OrderCancellationError),
    #[error("duplicated order")]
    DuplicatedOrder,
    #[error("{0:?}")]
    OrderValidation(ValidationError),
    #[error("database error: {0}")]
    Database(#[from] anyhow::Error),
    #[error("invalid appData format")]
    InvalidAppData(#[source] anyhow::Error),
    #[error("the new order is not a valid replacement for the old one")]
    InvalidReplacement,
    #[error(
        "contract app data {contract_app_data:?} is associated with full app data {existing:?} \
         which is different from the provided {provided:?}"
    )]
    AppDataMismatch {
        contract_app_data: AppDataHash,
        provided: String,
        existing: String,
    },
    #[error("quote metadata failed to serialize as json")]
    MetadataSerializationFailed,
}

impl AddOrderError {
    fn from_insertion(err: InsertionError, order: &Order) -> Self {
        match err {
            InsertionError::DuplicatedRecord => AddOrderError::DuplicatedOrder,
            InsertionError::DbError(err) => AddOrderError::Database(err.into()),
            InsertionError::AppDataMismatch(existing) => AddOrderError::AppDataMismatch {
                contract_app_data: order.data.app_data,
                // Unwrap because this error can only occur if full app data was set.
                provided: order.metadata.full_app_data.clone().unwrap(),
                // Unwrap because we only store utf-8 full app data.
                existing: {
                    let s = String::from_utf8_lossy(&existing);
                    if let Cow::Owned(_) = s {
                        tracing::error!(uid=%order.metadata.uid, "app data is not utf-8")
                    }
                    s.into_owned()
                },
            },
            InsertionError::MetadataSerializationFailed => {
                AddOrderError::MetadataSerializationFailed
            }
        }
    }
}

// This requires a manual implementation because the `#[from]` attribute from
// `thiserror` implies `#[source]` which requires `ValidationError: Error`,
// which it currently does not!
impl From<ValidationError> for AddOrderError {
    fn from(err: ValidationError) -> Self {
        Self::OrderValidation(err)
    }
}

#[derive(Debug, Error)]
pub enum OrderCancellationError {
    #[error("invalid signature")]
    InvalidSignature,
    #[error("signer does not match order owner")]
    WrongOwner,
    #[error("order not found")]
    OrderNotFound,
    #[error("order already cancelled")]
    AlreadyCancelled,
    #[error("order fully executed")]
    OrderFullyExecuted,
    #[error("order expired")]
    OrderExpired,
    #[error("on-chain orders cannot be cancelled with off-chain signature")]
    OnChainOrder,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub struct Orderbook {
    domain_separator: DomainSeparator,
    settlement_contract: H160,
    database: crate::database::Postgres,
    order_validator: Arc<dyn OrderValidating>,
    app_data: Arc<crate::app_data::Registry>,
}

impl Orderbook {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        domain_separator: DomainSeparator,
        settlement_contract: H160,
        database: crate::database::Postgres,
        order_validator: Arc<dyn OrderValidating>,
        app_data: Arc<crate::app_data::Registry>,
    ) -> Self {
        Metrics::initialize();
        Self {
            domain_separator,
            settlement_contract,
            database,
            order_validator,
            app_data,
        }
    }

    pub async fn add_order(
        &self,
        payload: OrderCreation,
    ) -> Result<(OrderUid, Option<QuoteId>), AddOrderError> {
        let full_app_data_override = match payload.app_data {
            OrderCreationAppData::Hash { hash } => self.app_data.find(&hash).await?,
            _ => None,
        };

        let replaced_order = self
            .get_replaced_order(&payload, full_app_data_override.as_deref())
            .await?;

        let (order, quote) = self
            .order_validator
            .validate_and_construct_order(
                payload,
                &self.domain_separator,
                self.settlement_contract,
                full_app_data_override,
            )
            .await?;

        // Check if it has to replace an existing order
        if let Some(old_order) = replaced_order {
            self.replace_order(order, old_order, quote).await
        } else {
            let quote_id = quote.as_ref().and_then(|quote| quote.id);

            self.database
                .insert_order(&order, quote.clone())
                .await
                .map_err(|err| AddOrderError::from_insertion(err, &order))?;
            Metrics::on_order_operation(
                &OrderWithQuote::try_new(order.clone(), quote)?,
                OrderOperation::Created,
            );

            Ok((order.metadata.uid, quote_id))
        }
    }

    /// Finds an order for cancellation.
    ///
    /// Returns an error if the order cannot be found or cannot be cancelled.
    async fn find_order_for_cancellation(
        &self,
        order_uid: &OrderUid,
    ) -> Result<OrderWithQuote, OrderCancellationError> {
        let order = self
            .database
            .single_order_with_quote(order_uid)
            .await?
            .ok_or(OrderCancellationError::OrderNotFound)?;

        match order.order.metadata.status {
            OrderStatus::PresignaturePending => return Err(OrderCancellationError::OnChainOrder),
            OrderStatus::Open if !order.order.signature.scheme().is_ecdsa_scheme() => {
                return Err(OrderCancellationError::OnChainOrder);
            }
            OrderStatus::Fulfilled => return Err(OrderCancellationError::OrderFullyExecuted),
            OrderStatus::Cancelled => return Err(OrderCancellationError::AlreadyCancelled),
            OrderStatus::Expired => return Err(OrderCancellationError::OrderExpired),
            _ => {}
        }

        Ok(order)
    }

    pub async fn cancel_orders(
        &self,
        cancellation: SignedOrderCancellations,
    ) -> Result<(), OrderCancellationError> {
        let mut orders = Vec::new();
        for order_uid in &cancellation.data.order_uids {
            orders.push(self.find_order_for_cancellation(order_uid).await?);
        }

        // Verify the cancellation signer is the same as the order signers
        let signer = cancellation
            .validate(&self.domain_separator)
            .map_err(|_| OrderCancellationError::InvalidSignature)?;
        if orders
            .iter()
            .any(|order| signer != order.order.metadata.owner)
        {
            return Err(OrderCancellationError::WrongOwner);
        };

        // orders are already known to exist in DB at this point, and signer is
        // known to be correct!
        self.database
            .cancel_orders(cancellation.data.order_uids, Utc::now())
            .await?;

        for order in &orders {
            tracing::debug!(order_uid =% order.order.metadata.uid, "order cancelled");
            Metrics::on_order_operation(order, OrderOperation::Cancelled);
        }

        Ok(())
    }

    pub async fn cancel_order(
        &self,
        cancellation: OrderCancellation,
    ) -> Result<(), OrderCancellationError> {
        let order = self
            .find_order_for_cancellation(&cancellation.order_uid)
            .await?;

        // Verify the cancellation signer is the same as the order signer.
        let signer = cancellation
            .validate(&self.domain_separator)
            .map_err(|_| OrderCancellationError::InvalidSignature)?;
        if signer != order.order.metadata.owner {
            return Err(OrderCancellationError::WrongOwner);
        };

        // order is already known to exist in DB at this point, and signer is
        // known to be correct!
        self.database
            .cancel_order(&order.order.metadata.uid, Utc::now())
            .await?;

        tracing::debug!(order_uid =% order.order.metadata.uid, "order cancelled");
        Metrics::on_order_operation(&order, OrderOperation::Cancelled);

        Ok(())
    }

    async fn get_replaced_order(
        &self,
        new_order: &OrderCreation,
        app_data_override: Option<&str>,
    ) -> Result<Option<OrderWithQuote>, AddOrderError> {
        let full_app_data = match &new_order.app_data {
            OrderCreationAppData::Hash { .. } => app_data_override,
            OrderCreationAppData::Both { full, .. } | OrderCreationAppData::Full { full } => {
                Some(full.as_str())
            }
        };

        if let Some(full_app_data) = full_app_data {
            let validated_app_data = Validator::new(usize::MAX)
                .validate(full_app_data.as_bytes())
                .map_err(AddOrderError::InvalidAppData)?;

            if let Some(replaced_order) = validated_app_data.protocol.replaced_order {
                return Ok(Some(
                    self.find_order_for_cancellation(&replaced_order.uid.into())
                        .await
                        .map_err(AddOrderError::OrderNotFound)?,
                ));
            }
        }
        Ok(None)
    }

    pub async fn replace_order(
        &self,
        validated_new_order: Order,
        old_order: OrderWithQuote,
        quote: Option<Quote>,
    ) -> Result<(OrderUid, Option<i64>), AddOrderError> {
        // Replacement order signatures need to be validated meaning we cannot
        // accept `PreSign` orders, otherwise anyone can cancel a user order by
        // submitting a `PreSign` order on someone's behalf.
        validated_new_order
            .signature
            .scheme()
            .try_to_ecdsa_scheme()
            .ok_or(AddOrderError::InvalidReplacement)?;

        // Verify that the new order is a valid replacement order by checking
        // that both the old and new orders have the same signer.
        if validated_new_order.metadata.owner != old_order.order.metadata.owner {
            return Err(AddOrderError::InvalidReplacement);
        }

        let quote_id = quote.as_ref().and_then(|quote| quote.id);

        self.database
            .replace_order(
                &old_order.order.metadata.uid,
                &validated_new_order,
                quote.clone(),
            )
            .await
            .map_err(|err| AddOrderError::from_insertion(err, &validated_new_order))?;
        Metrics::on_order_operation(&old_order, OrderOperation::Cancelled);
        Metrics::on_order_operation(
            &OrderWithQuote::try_new(validated_new_order.clone(), quote)?,
            OrderOperation::Created,
        );

        Ok((validated_new_order.metadata.uid, quote_id))
    }

    pub async fn get_order(&self, uid: &OrderUid) -> Result<Option<Order>> {
        self.database.single_order(uid).await
    }

    pub async fn get_orders_for_tx(&self, hash: &H256) -> Result<Vec<Order>> {
        self.database.orders_for_tx(hash).await
    }

    pub async fn get_auction(&self) -> Result<Option<dto::AuctionWithId>> {
        let auction = match self.database.most_recent_auction().await? {
            Some(auction) => auction,
            None => {
                tracing::warn!("there is no current auction");
                return Ok(None);
            }
        };
        Ok(Some(auction))
    }

    pub async fn get_user_orders(
        &self,
        owner: &H160,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<Order>> {
        self.database
            .user_orders(owner, offset, Some(limit))
            .await
            .context("get_user_orders error")
    }

    pub async fn get_order_status(&self, uid: &OrderUid) -> Result<Option<dto::order::Status>> {
        let solutions = |competition: SolverCompetitionAPI| {
            competition
                .common
                .solutions
                .into_iter()
                .map(|solution| {
                    let executed_amounts = solution.orders.iter().find_map(|o| match o {
                        solver_competition::Order::Legacy { .. } => None,
                        solver_competition::Order::Colocated {
                            id,
                            sell_amount,
                            buy_amount,
                        } => (id == uid).then_some(dto::order::ExecutedAmounts {
                            sell: *sell_amount,
                            buy: *buy_amount,
                        }),
                    });
                    dto::order::SolutionInclusion {
                        solver: solution.solver,
                        executed_amounts,
                    }
                })
                .collect::<Vec<_>>()
        };

        let latest_competition = async {
            let competition = self.database.load_latest_competition().await?;
            Ok::<_, anyhow::Error>(solutions(competition))
        };

        // Once an order was executed we always want to return `Traded` with the
        // competition data of the **first** time it was traded for a stable result.
        // Under some circumstances it can happen that the latest state of an already
        // executed order is not `Traded`. To detect that we first check the trades
        // table and return the appropriate competition data.
        let trades = self
            .database
            .trades(&TradeFilter {
                owner: None,
                order_uid: Some(*uid),
            })
            .await?;

        match trades.first().map(|trade| trade.tx_hash) {
            Some(Some(tx_hash)) => {
                let competition = self
                    .database
                    .load_competition(Identifier::Transaction(tx_hash))
                    .await?;
                return Ok(Some(dto::order::Status::Traded(solutions(competition))));
            }
            // order executed but not fully indexed and processed
            Some(None) => {
                return Ok(Some(dto::order::Status::Traded(latest_competition.await?)));
            }
            None => (),
        }

        let latest_event = self.database.latest_order_event(uid).await?;
        let status = match latest_event.context("no event")?.label {
            OrderEventLabel::Ready => dto::order::Status::Active,
            OrderEventLabel::Created => dto::order::Status::Scheduled,
            OrderEventLabel::Considered => dto::order::Status::Solved(latest_competition.await?),
            OrderEventLabel::Executing => dto::order::Status::Executing(latest_competition.await?),
            // order executed but not fully indexed and processed
            OrderEventLabel::Traded => dto::order::Status::Traded(latest_competition.await?),
            OrderEventLabel::Cancelled => dto::order::Status::Cancelled,
            OrderEventLabel::Filtered => dto::order::Status::Open,
            OrderEventLabel::Invalid => dto::order::Status::Open,
        };
        Ok(Some(status))
    }
}

#[async_trait::async_trait]
impl LivenessChecking for Orderbook {
    async fn is_alive(&self) -> bool {
        self.get_auction().await.is_ok()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::database::orders::MockOrderStoring,
        ethcontract::H160,
        mockall::predicate::eq,
        model::{
            order::{OrderData, OrderMetadata},
            signature::Signature,
        },
        shared::order_validation::MockOrderValidating,
    };

    #[tokio::test]
    #[ignore]
    async fn postgres_replace_order_verifies_signer_and_app_data() {
        let old_order = Order {
            metadata: OrderMetadata {
                uid: OrderUid([1; 56]),
                owner: H160([1; 20]),
                ..Default::default()
            },
            data: OrderData {
                valid_to: u32::MAX,
                ..Default::default()
            },
            ..Default::default()
        };
        let new_order_uid = OrderUid([2; 56]);

        let mut database = MockOrderStoring::new();
        database
            .expect_single_order()
            .with(eq(old_order.metadata.uid))
            .returning({
                let old_order = old_order.clone();
                move |_| Ok(Some(old_order.clone()))
            });
        database.expect_replace_order().returning(|_, _, _| Ok(()));

        let mut order_validator = MockOrderValidating::new();
        order_validator
            .expect_validate_and_construct_order()
            .returning(move |creation, _, _, _| {
                Ok((
                    Order {
                        metadata: OrderMetadata {
                            owner: creation.from.unwrap(),
                            uid: new_order_uid,
                            ..Default::default()
                        },
                        data: creation.data(),
                        signature: creation.signature,
                        ..Default::default()
                    },
                    Default::default(),
                ))
            });

        let database = crate::database::Postgres::new("postgresql://").unwrap();
        database::clear_DANGER(&database.pool).await.unwrap();
        database.insert_order(&old_order, None).await.unwrap();
        let app_data = Arc::new(crate::app_data::Registry::new(
            Validator::new(8192),
            database.clone(),
            None,
        ));
        let orderbook = Orderbook {
            database,
            order_validator: Arc::new(order_validator),
            domain_separator: Default::default(),
            settlement_contract: H160([0xba; 20]),
            app_data,
        };

        // Different owner
        assert!(matches!(
            orderbook
                .add_order(OrderCreation {
                    from: Some(H160([2; 20])),
                    signature: Signature::Eip712(Default::default()),
                    app_data: OrderCreationAppData::Full {
                        full: format!(
                            r#"{{"version":"1.1.0","metadata":{{"replacedOrder":{{"uid":"{}"}}}}}}"#,
                            old_order.metadata.uid
                        )
                    },
                    ..Default::default()
                },)
                .await,
            Err(AddOrderError::InvalidReplacement)
        ));

        // Different replacedOrder
        assert!(matches!(
            orderbook
                .add_order(OrderCreation {
                    from: Some(H160([2; 20])),
                    signature: Signature::Eip712(Default::default()),
                    app_data: OrderCreationAppData::Full {
                        full: format!(
                            r#"{{"version":"1.1.0","metadata":{{"replacedOrder":{{"uid":"{}"}}}}}}"#,
                            OrderUid::from_integer(1000),
                        )
                    },
                    ..Default::default()
                },)
                .await,
            Err(AddOrderError::OrderNotFound(
                OrderCancellationError::OrderNotFound
            ))
        ));

        // Non-signed order.
        assert!(matches!(
            orderbook
                .add_order(OrderCreation {
                    from: Some(old_order.metadata.owner),
                    signature: Signature::PreSign,
                    app_data: OrderCreationAppData::Full {
                        full: format!(
                            r#"{{"version":"1.1.0","metadata":{{"replacedOrder":{{"uid":"{}"}}}}}}"#,
                            old_order.metadata.uid
                        )
                    },
                    ..Default::default()
                },)
                .await,
            Err(AddOrderError::InvalidReplacement)
        ));

        // Stars align...
        let (order_id, _) = orderbook
            .add_order(OrderCreation {
                from: Some(old_order.metadata.owner),
                signature: Signature::Eip712(Default::default()),
                app_data: OrderCreationAppData::Full {
                    full: format!(
                        r#"{{"version":"1.1.0","metadata":{{"replacedOrder":{{"uid":"{}"}}}}}}"#,
                        old_order.metadata.uid
                    ),
                },
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(order_id, new_order_uid,);
    }
}
