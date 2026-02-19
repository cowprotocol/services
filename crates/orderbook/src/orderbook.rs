use {
    crate::{
        database::{
            orders::{InsertionError, OrderStoring},
            trades::{TradeFilter, TradeRetrieving},
        },
        dto,
        solver_competition::{Identifier, LoadSolverCompetitionError, SolverCompetitionStoring},
    },
    alloy::primitives::{Address, B256},
    anyhow::{Context, Result},
    app_data::{AppDataHash, Validator},
    bigdecimal::ToPrimitive,
    chrono::Utc,
    database::order_events::OrderEventLabel,
    model::{
        DomainSeparator,
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
    },
    observe::metrics::LivenessChecking,
    shared::{
        fee::FeeParameters,
        order_quoting::Quote,
        order_validation::{
            Amounts,
            OrderValidating,
            ValidationError,
            is_order_outside_market_price,
        },
    },
    std::{borrow::Cow, sync::Arc},
    strum::Display,
    thiserror::Error,
    tracing::instrument,
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

    fn on_order_operation(order: &Order, operation: OrderOperation) {
        let class = if order.metadata.quote.as_ref().is_some_and(|quote| {
            // Check if the order at the submission time was "in market"
            !is_order_outside_market_price(
                &Amounts {
                    sell: order.data.sell_amount,
                    buy: order.data.buy_amount,
                    fee: order.data.fee_amount,
                },
                &Amounts {
                    sell: quote.sell_amount,
                    buy: quote.buy_amount,
                    fee: FeeParameters {
                        // safe to unwrap as these values were converted from f64 previously
                        gas_amount: quote.gas_amount.to_f64().unwrap(),
                        gas_price: quote.gas_price.to_f64().unwrap(),
                        sell_token_price: quote.sell_token_price.to_f64().unwrap(),
                    }
                    .fee(),
                },
                order.data.kind,
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
    #[error("the new order is not a valid replacement for the old one: {0}")]
    InvalidReplacement(#[source] OrderReplacementError),
    #[error(
        "contract app data {contract_app_data:?} is associated with full app data {existing:?} \
         which is different from the provided {provided:?}"
    )]
    AppDataMismatch {
        contract_app_data: AppDataHash,
        provided: String,
        existing: String,
    },
    #[error("quote metadata failed to serialize as json, error: {0}")]
    MetadataSerializationFailed(serde_json::Error),
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
            InsertionError::MetadataSerializationFailed(err) => {
                AddOrderError::MetadataSerializationFailed(err)
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

#[derive(Debug, Error)]
pub enum OrderReplacementError {
    #[error("invalid signature")]
    InvalidSignature,
    #[error("signer does not match older order owner")]
    WrongOwner,
    #[error("old order is actively being bid on")]
    OldOrderActivelyBidOn,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug)]
pub struct QuoteMetadata {
    pub id: Option<QuoteId>,
    pub solver: Address,
}

impl From<&Quote> for QuoteMetadata {
    fn from(value: &Quote) -> Self {
        Self {
            id: value.id,
            solver: value.data.solver,
        }
    }
}

pub struct Orderbook {
    domain_separator: DomainSeparator,
    settlement_contract: Address,
    database: crate::database::Postgres,
    database_replica: crate::database::Postgres,
    order_validator: Arc<dyn OrderValidating>,
    app_data: Arc<crate::app_data::Registry>,
    active_order_competition_threshold: u32,
}

impl Orderbook {
    pub fn new(
        domain_separator: DomainSeparator,
        settlement_contract: Address,
        database: crate::database::Postgres,
        database_replica: crate::database::Postgres,
        order_validator: Arc<dyn OrderValidating>,
        app_data: Arc<crate::app_data::Registry>,
        active_order_competition_threshold: u32,
    ) -> Self {
        Metrics::initialize();
        Self {
            domain_separator,
            settlement_contract,
            database,
            database_replica,
            order_validator,
            app_data,
            active_order_competition_threshold,
        }
    }

    /// Validates and stores an order in the database.
    ///
    /// 1. If the provided app data is a hash (instead of a complete app data
    ///    JSON), retrieves it from the database.
    /// 3. Validates and constructs an order.
    /// 4. If the new order is to replace an old one, replaces it; otherwise,
    ///    the new order is simply inserted in the database
    #[instrument(skip_all)]
    pub async fn add_order(
        &self,
        payload: OrderCreation,
    ) -> Result<(OrderUid, Option<QuoteMetadata>), AddOrderError> {
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

        let order_uid = order.metadata.uid;

        // Check if it has to replace an existing order
        if let Some(old_order) = replaced_order {
            self.replace_order(order, old_order).await?
        } else {
            self.database
                .insert_order(&order)
                .await
                .map_err(|err| AddOrderError::from_insertion(err, &order))?;
            Metrics::on_order_operation(&order, OrderOperation::Created);
        }

        Ok((order_uid, quote.as_ref().map(QuoteMetadata::from)))
    }

    /// Finds an order for cancellation.
    ///
    /// Returns an error if the order cannot be found or cannot be cancelled
    /// (for example, orders using PreSign).
    async fn find_order_for_cancellation(
        &self,
        order_uid: &OrderUid,
    ) -> Result<Order, OrderCancellationError> {
        let order = self
            .database
            .single_order(order_uid)
            .await?
            .ok_or(OrderCancellationError::OrderNotFound)?;

        match order.metadata.status {
            OrderStatus::PresignaturePending => return Err(OrderCancellationError::OnChainOrder),
            OrderStatus::Open if !order.signature.scheme().is_ecdsa_scheme() => {
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
        if orders.iter().any(|order| signer != order.metadata.owner) {
            return Err(OrderCancellationError::WrongOwner);
        };

        // orders are already known to exist in DB at this point, and signer is
        // known to be correct!
        self.database
            .cancel_orders(cancellation.data.order_uids, Utc::now())
            .await?;

        for order in &orders {
            tracing::debug!(order_uid =% order.metadata.uid, "order cancelled");
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
        if signer != order.metadata.owner {
            return Err(OrderCancellationError::WrongOwner);
        };

        // order is already known to exist in DB at this point, and signer is
        // known to be correct!
        self.database
            .cancel_order(&order.metadata.uid, Utc::now())
            .await?;

        tracing::debug!(order_uid =% order.metadata.uid, "order cancelled");
        Metrics::on_order_operation(&order, OrderOperation::Cancelled);

        Ok(())
    }

    /// Using the provided app data, finds the order to be replaced.
    ///
    /// Validates the provided app data before searching for the order to be
    /// replaced.
    async fn get_replaced_order(
        &self,
        new_order: &OrderCreation,
        app_data_override: Option<&str>,
    ) -> Result<Option<Order>, AddOrderError> {
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

    /// Replaces the `old_order` with `validated_new_order` in the database
    /// (cancels the old one and inserts the new one), but not before
    /// performing some validations.
    ///
    /// 1. New order's signature cannot be a `PreSign` order to avoid users
    ///    cancelling orders on someone else's behalf.
    /// 2. Old and new order MUST have the same signer.
    /// 3. The old order cannot be bid on (to prevent double spending).
    pub async fn replace_order(
        &self,
        validated_new_order: Order,
        old_order: Order,
    ) -> Result<(), AddOrderError> {
        validated_new_order
            .signature
            .scheme()
            .try_to_ecdsa_scheme()
            .ok_or(AddOrderError::InvalidReplacement(
                OrderReplacementError::InvalidSignature,
            ))?;

        if validated_new_order.metadata.owner != old_order.metadata.owner {
            return Err(AddOrderError::InvalidReplacement(
                OrderReplacementError::WrongOwner,
            ));
        }

        if self
            .order_is_actively_bid_on(old_order.metadata.uid)
            .await?
        {
            return Err(AddOrderError::InvalidReplacement(
                OrderReplacementError::OldOrderActivelyBidOn,
            ));
        }

        self.database
            .replace_order(&old_order.metadata.uid, &validated_new_order)
            .await
            .map_err(|err| AddOrderError::from_insertion(err, &validated_new_order))?;
        Metrics::on_order_operation(&old_order, OrderOperation::Cancelled);
        Metrics::on_order_operation(&validated_new_order, OrderOperation::Created);

        Ok(())
    }

    async fn order_is_actively_bid_on(&self, order_uid: OrderUid) -> Result<bool> {
        let latest_competitions = self
            .database
            .load_latest_competitions(self.active_order_competition_threshold)
            .await?;

        let order_is_bid_on = latest_competitions
            .into_iter()
            .flat_map(|competition| competition.common.solutions)
            .flat_map(|solution| solution.orders)
            .map(|order| match order {
                solver_competition::Order::Colocated { id, .. } => id,
                solver_competition::Order::Legacy { id, .. } => id,
            })
            .any(|uid| uid == order_uid);

        Ok(order_is_bid_on)
    }

    pub async fn get_order(&self, uid: &OrderUid) -> Result<Option<Order>> {
        self.database_replica.single_order(uid).await
    }

    pub async fn get_orders_for_tx(&self, hash: &B256) -> Result<Vec<Order>> {
        self.database_replica.orders_for_tx(hash).await
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
        owner: &Address,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<Order>> {
        self.database_replica
            .user_orders(owner, offset, Some(limit))
            .await
            .context("get_user_orders error")
    }

    pub async fn get_order_status(
        &self,
        uid: &OrderUid,
    ) -> Result<dto::order::Status, OrderStatusError> {
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
            let competition = SolverCompetitionStoring::load_latest_competition(&self.database)
                .await
                .map_err(Into::<OrderStatusError>::into)?;
            Ok::<_, OrderStatusError>(solutions(competition))
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
                return Ok(dto::order::Status::Traded(solutions(competition)));
            }
            // order executed but not fully indexed and processed
            Some(None) => {
                return Ok(dto::order::Status::Traded(latest_competition.await?));
            }
            None => (),
        }

        let latest_event = self.database.latest_order_event(uid).await?;
        let status = match latest_event.ok_or(OrderStatusError::NotFound)?.label {
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
        Ok(status)
    }
}

#[derive(Error, Debug)]
pub enum OrderStatusError {
    #[error("order status not found")]
    NotFound,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<LoadSolverCompetitionError> for OrderStatusError {
    fn from(value: LoadSolverCompetitionError) -> Self {
        match value {
            LoadSolverCompetitionError::NotFound => Self::NotFound,
            LoadSolverCompetitionError::Other(err) => Self::Other(err),
        }
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
                owner: Address::new([1; 20]),
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
        database.expect_replace_order().returning(|_, _| Ok(()));

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

        let database =
            crate::database::Postgres::try_new("postgresql://", Default::default()).unwrap();
        database::clear_DANGER(&database.pool).await.unwrap();
        database.insert_order(&old_order).await.unwrap();

        let database_replica = database.clone();
        let app_data = Arc::new(crate::app_data::Registry::new(
            Validator::new(8192),
            database.clone(),
            None,
        ));
        let orderbook = Orderbook {
            database,
            database_replica,
            order_validator: Arc::new(order_validator),
            domain_separator: Default::default(),
            settlement_contract: Address::repeat_byte(0xba),
            app_data,
            active_order_competition_threshold: Default::default(),
        };

        // Different owner
        assert!(matches!(
            orderbook
                .add_order(OrderCreation {
                    from: Some(Address::repeat_byte(2)),
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
            Err(AddOrderError::InvalidReplacement(
                OrderReplacementError::WrongOwner
            ))
        ));

        // Different replacedOrder
        assert!(matches!(
            orderbook
                .add_order(OrderCreation {
                    from: Some(Address::repeat_byte(2)),
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
            Err(AddOrderError::InvalidReplacement(
                OrderReplacementError::InvalidSignature
            ))
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
